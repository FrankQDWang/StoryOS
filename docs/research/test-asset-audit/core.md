# Core tests — partial review

Completed: five Project and Volume test files, 22 tests. The directory is not complete. Reason codes: [METHOD.md](METHOD.md).

D5 does not claim that the named public test executes an unreachable guard. Record an uncovered mutation if selected; do not present it as a successful coverage experiment.

## crates/storyos-core/src/archive_project_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO003 | 16 — a_matching_revision_and_active_lifecycle_classifies_as_applied | DELETE | D1 | The HTTP test asserts an Applied archive, revision 2 and archived library state. The Core success tuple adds no input or effect. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| CO004 | 24 — a_stale_revision_classifies_as_conflicted_with_zero_lifecycle_effect | DELETE | D1 | The HTTP test archives at revision 1, then submits a new key at stale revision 1 and expects Conflicted. The current Project is already archived, so this also observes stale-revision precedence. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| CO005 | 37 — an_already_archived_project_classifies_as_no_effect | DELETE | D1 | The HTTP test sends a new key at revision 2 to an already archived Project and expects NoEffect. It exercises the same Core lifecycle condition. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| CO006 | 49 — a_missing_project_classifies_as_refused_with_zero_lifecycle_effect | DELETE | D5 | Only persist_archive_project calls this classifier in product code. It returns MissingProject before the call and always passes ProjectPresence::Present (archive_project.rs:83,101 in the adapter). Absent is an internal synthetic input here. HTTP Scope refusal remains covered, but that test cannot kill a mutation confined to this unreachable Core guard. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |

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

