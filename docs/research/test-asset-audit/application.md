# application test verdicts

Reviewed: 58 cases in 20 files. See PROGRESS.md for directory completion.

Reason codes: [METHOD.md](METHOD.md). Locations use the fixed audit baseline.

## crates/storyos-application/src/archive_project_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP007 | 68 — an_exact_archive_project_binding_reaches_the_store | DELETE | D1 | Public Archive observes the actual lifecycle/revision and replay; the local fake merely returns Applied revision 2 and counts the pass-through Store call. Execution order: keep the cited covering test until NP030 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| AP008 | 79 — a_changed_archive_project_binding_is_refused_before_the_store | DELETE | D5 | Server archive_project.rs constructs command_kind=archiveProject from a literal. The local changed-kind command has no current request path; public binding checks protect actual challenge substitution. Execution order: keep the cited covering test until NP030 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| AP009 | 91 — a_changed_retry_digest_is_refused_before_the_store | DELETE | D5 | Server derives in-memory canonical bytes and digest together. This fixture changes only bytes, unlike a real changed retry whose supplied request is self-consistent and conflicts with stored Admission. The named public test does not execute this synthetic mismatch guard. Execution order: keep the cited covering test until NP030 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| AP010 | 103 — an_invalid_expected_revision_is_refused_before_the_store | DELETE | D5 | The Server parses expected Project revision and requires it to be positive before the Application call. A direct expected_revision=0 fixture cannot reach this guard from the current endpoint. Execution order: keep the cited covering test until NP030 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |

## crates/storyos-application/src/author_command_outcome_unknown_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP056 | 21 — append_delegates_once_and_returns_the_complete_immutable_observation | DELETE | D1 | The Adapter append test exercises real immutable observation identity, exact serialization, concurrent sequence allocation and zero authority change through the same Application function. The fake echo and call count add no observed product invariant. | crates/storyos-adapter-postgres/src/author_command_outcome_unknown_tests.rs:108 |

## crates/storyos-application/src/author_edit_outcome_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP057 | 41 — outcome_query_resolves_once_without_retrying_a_command | DELETE | D1 | The Adapter outcome test resolves an unexpired challenge as unknown and proves the query does not consume it; its concurrent consume test proves no false rejection. The fake resolver merely echoes a constant unknown observation, without a command capability to retry. | apps/web/test/node-postgresql/project-http.integration.test.ts:253; crates/storyos-adapter-postgres/src/author_edit_outcome_tests.rs:229 |

## crates/storyos-application/src/author_edit_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP020 | 107 — matching_bindings_reach_the_atomic_store_once | DELETE | D1 | The fake Store always returns Base! even after a second primitive is added, so it checks only one call and forwarding. Public Project author edit actually applies two ordered units and reads the settled Chapter and Receipt. | apps/web/test/node-postgresql/project-http.integration.test.ts:253 |
| AP021 | 147 — scope_substitution_fails_before_the_store | DELETE | D5 | Server author_edit.rs clones the same authenticated scope into command and challenge. The fixture independently replaces only challenge.project_id, which cannot come from the current handler. Public foreign-scope/challenge tests own the actual authorization behavior. | apps/web/test/node-postgresql/project-http.integration.test.ts:253 |
| AP022 | 159 — canonical_payload_substitution_fails_before_the_store | DELETE | D5 | Server computes the command digest from its canonical bytes before Application dispatch. The fixture changes bytes alone to an invalid escaped JSON fragment and never invokes the request parser. Public request substitution is a different stored-Admission check; no kill of this isolated redundant guard is claimed. | apps/web/test/node-postgresql/project-http.integration.test.ts:253 |

## crates/storyos-application/src/chapter_query_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP023 | 59 — a_later_in_scope_chapter_returns_its_snapshot_and_head | DELETE | D1 | Public Create Chapter opens later Chapters B/C by identity and checks their title, exact Head body and canonical tree. The fixture's echo adds no distinct lookup behavior. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |
| AP024 | 80 — a_chapter_outside_project_scope_fails_closed | DELETE | D5 | The only product ChapterQueryReader constructs facts.project_scope by cloning the requested scope after its scoped SQL query. A foreign scope inside otherwise found facts is synthetic. Public foreign access is covered, but does not execute the redundant inconsistent-facts branch. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |
| AP025 | 98 — an_expired_snapshot_fails_closed | DELETE | D1 | The public Chapter query expires the retained Snapshot and requires HTTP 409 snapshot_expired. This includes forwarding the Adapter expiration result through Application. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |
| AP026 | 113 — a_missing_chapter_fails_closed | DELETE | D1 | The scoped Adapter query proves a Chapter outside the requested Project returns Missing through open_chapter. The fake Missing enum only repeats that pass-through behavior. | crates/storyos-adapter-postgres/tests/project_scope.rs:35 |
| AP027 | 128 — a_chapter_identity_mismatch_fails_closed | DELETE | D5 | The Adapter selects manuscript_object_id by the requested Chapter ID and returns that selected ID. A Reader returning Chapter B for requested Chapter A is not current product input. The public exact-ID lookup owns real Chapter selection, not the synthetic mismatch guard. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |

## crates/storyos-application/src/create_agent_run_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP015 | 75 — rejects_a_digest_mismatch_before_the_store | DELETE | D5 | Server create_agent_run.rs derives the canonical digest from the same canonical bytes used in the command. The local literal wrong digest is not a possible parsed request. Public createAgentRun changed-retry refusal tests the actual stored binding, not this synthetic in-memory mismatch. | apps/web/test/node-postgresql/create-agent-run-http.integration.test.ts:626 |

## crates/storyos-application/src/create_project_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP001 | 64 — an_exact_create_project_binding_reaches_the_store | DELETE | D1 | The fake Store returns the supplied title and counts one call. Public Create Project asserts the new Project, canonical title, durable acknowledgement and exact retry, covering useful dispatch and result propagation. | apps/web/test/node-postgresql/create-project-http.integration.test.ts:75 |
| AP002 | 72 — a_changed_create_project_binding_is_refused_before_the_store | DELETE | D5 | Server create_project.rs supplies command_kind=createProject as a literal when constructing the command. The test mutates this internal value without changing the route. Public challenge substitution is checked against stored Admission evidence, not this unreachable wrong-kind tuple. | apps/web/test/node-postgresql/create-project-http.integration.test.ts:75 |

## crates/storyos-application/src/editor_session_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP016 | 55 — use_cases_preserve_the_exact_session_binding | DELETE | D1 | Public Project flow creates and reads an EditorSession and uses that session for real author edits. The local Reader ignores scope and returns its own fixture, so equality only tests forwarding and cannot establish storage isolation. | apps/web/test/node-postgresql/project-http.integration.test.ts:253 |
| AP017 | 104 — create_refuses_mismatched_challenge_and_session_bindings | DELETE | D5 | Server editor_session.rs puts one computed binding_ref and the same session generation into both client and challenge fields. A direct mismatch is not a current request input; real cross-session challenge substitution is checked against stored evidence. | apps/web/test/node-postgresql/project-http.integration.test.ts:253 |

## crates/storyos-application/src/manuscript_search_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP036 | 82 — a_manuscript_search_returns_ordered_matches_with_chapter_block_and_range_identity | DELETE | D5 | Postgres emits Chapter facts in canonical Volume/Chapter order and assigns their order fields monotonically. This fixture reverses facts to exercise a redundant Application sort. Browser search owns actual Chapter/Block/range projection, and the retained Unicode case owns multiple exact match offsets; neither is claimed to kill removal of the synthetic re-sort alone. | apps/web/test/browser-exact-dist/s2-search.integration.test.ts:127; crates/storyos-application/src/manuscript_search_tests.rs:341 |
| AP037 | 144 — a_current_chapter_search_excludes_other_live_chapters | DELETE | D5 | CurrentChapter production reads are already filtered by Chapter ID in Postgres, so the two-Chapter fake input is not emitted for this selection. The browser switches Chapters and proves current search excludes the other Chapter; a mutation confined to the second internal filter is not claimed to be covered. | apps/web/test/browser-exact-dist/s2-search.integration.test.ts:127 |
| AP038 | 200 — zero_matches_are_a_complete_ready_page_not_projection_not_ready | DELETE | D1 | The packaged search uses a nonmatching query against written text and checks a complete zero-match result. The public HTTP test also checks a ready empty page, distinct from not-ready. | apps/web/test/browser-exact-dist/s2-search.integration.test.ts:127; apps/web/test/node-postgresql/manuscript-search-http.integration.test.ts:78 |
| AP039 | 234 — an_unmet_required_watermark_is_projection_not_ready | DELETE | D1 | HTTP search supplies required_watermark=current+1 and requires projection_not_ready. This executes the same Application branch with a real Snapshot. | apps/web/test/node-postgresql/manuscript-search-http.integration.test.ts:78 |
| AP040 | 269 — a_rebuild_from_the_same_canonical_facts_returns_the_same_page | DELETE | D1 | The scoped Adapter integration queries the same canonical facts twice and compares whole pages, then checks that search did not advance the watermark. The pure fixture equality adds no distinct rebuild behavior. | crates/storyos-adapter-postgres/src/manuscript_search_tests.rs:82 |
| AP041 | 289 — foreign_scope_facts_fail_closed | DELETE | D5 | Postgres clones requested scope into all returned search facts and uses scoped SQL. A fake top-level foreign scope cannot be produced by the current Reader; real cross-scope search refusal remains covered at HTTP. | apps/web/test/node-postgresql/manuscript-search-http.integration.test.ts:78 |
| AP042 | 309 — a_bounded_page_stops_at_the_absolute_item_limit | DELETE | D1 | The retained dense-page test exceeds the same 500-item limit and checks the exact count, Truncated flag, first and last match coordinates. This shorter x-only input adds no independent cap branch. | crates/storyos-application/src/manuscript_search_tests.rs:388 |
| AP043 | 341 — astral_unicode_before_and_inside_matches_keeps_exact_utf16_ranges | KEEP | K1 | Astral text before and inside two matches requires cumulative UTF-16 offsets 3..6 and 10..13. Byte/scalar offsets or double-counting between matches would select wrong editor text. Browser search uses simple one-match tokens, while the dense performance case is ASCII-only. | apps/web/test/browser-exact-dist/s2-search.integration.test.ts:127; crates/storyos-application/src/manuscript_search_tests.rs:388 |
| AP044 | 388 — dense_ascii_page_decodes_each_utf16_unit_once | KEEP | K1 | Restarting UTF-16 counting from the Block start for each match causes repeated work on a million-character Chapter. The hook counts actual units submitted to utf16_len and this test bounds the current scan at 997502 units while checking cap and coordinates. Output-only search tests cannot detect that repeated scan. This is an operation-count invariant, not a measured latency claim. | crates/storyos-application/src/manuscript_search_tests.rs:341; apps/web/test/browser-exact-dist/s2-search.integration.test.ts:127 |

## crates/storyos-application/src/manuscript_statistics_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP032 | 71 — chapter_and_manuscript_counts_use_the_same_unicode_profile | DELETE | D1 | The packaged browser writes the same Hello world and CJK Chapter text, switches current Chapter, and checks current and total word/character/Chapter counts plus profile and watermark. | apps/web/test/browser-exact-dist/s2-statistics.integration.test.ts:120 |
| AP033 | 126 — unmet_required_watermark_is_not_an_empty_success | KEEP | K1 | A statistics request requiring a watermark above the captured Snapshot must report ProjectionNotReady, not fresh zero counts. The browser scenario sends ordinary current queries. Search has a separate evaluate_search watermark branch, so its public refusal does not cover this statistics branch. | apps/web/test/browser-exact-dist/s2-statistics.integration.test.ts:120; apps/web/test/node-postgresql/manuscript-search-http.integration.test.ts:78 |
| AP034 | 158 — omitted_chapters_do_not_contribute_invented_counts | DELETE | D1 | After deleting a real Chapter, the packaged statistics journey checks remaining Chapter and manuscript totals without the deleted Chapter. A one-element fixture cannot add a more specific omission guarantee. | apps/web/test/browser-exact-dist/s2-statistics.integration.test.ts:120 |

## crates/storyos-application/src/manuscript_tree_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP028 | 46 — an_empty_active_project_returns_an_ordered_canonical_tree_with_zero_volumes_and_zero_chapters | DELETE | D1 | The public empty-tree query checks zero Volumes and Chapters with canonical Snapshot and tree revision, covering the fake empty tree projection. | apps/web/test/node-postgresql/manuscript-tree-http.integration.test.ts:72 |
| AP029 | 73 — a_snapshot_outside_project_scope_returns_no_canonical_tree | DELETE | D5 | Postgres tree loading clones request scope into its top-level facts. Foreign access is denied by scoped loading; the fixture's contradictory returned scope has no current product producer. | apps/web/test/node-postgresql/manuscript-tree-http.integration.test.ts:72 |
| AP030 | 90 — a_volume_outside_project_scope_returns_no_canonical_tree | DELETE | D5 | Every VolumeFact receives the requested scope clone from the scoped Adapter query. A fake foreign Volume nested under owned facts is not a reachable response from that Reader; HTTP covers actual owner isolation. | apps/web/test/node-postgresql/manuscript-tree-http.integration.test.ts:72 |
| AP031 | 113 — a_chapter_outside_project_scope_returns_no_canonical_tree | DELETE | D5 | Every ChapterFact receives the requested scope clone and is selected by owner/project SQL. The foreign child scope here is fabricated independently of those product facts. | apps/web/test/node-postgresql/manuscript-tree-http.integration.test.ts:72 |

## crates/storyos-application/src/project_activity_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP058 | 4 — an_unknown_persisted_kind_fails_closed | DELETE | D2 | This asserts absence of one literal event kind from the Contracts-owned enum re-export. It tests no Application logic and would reject a deliberate future addition of that kind. Public Activity tests cover actual delivered kinds and order; they do not cover generic unknown-kind refusal. No separate covering test for this exact static absence was found, and no observable regression unique to forbidding this particular name was established. | apps/web/test/node-postgresql/activity-stream-cross-table-http.integration.test.ts:168 (actual supported Event behavior, not proof of this literal's absence) |

## crates/storyos-application/src/project_export_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP045 | 138 — an_exact_export_binding_reaches_the_store | DELETE | D1 | Public Project export admission checks real profiles, Snapshot, export identity and exact retry. The local fake only echoes those supplied fields. | apps/web/test/node-postgresql/project-export-admission-http.integration.test.ts:295 |
| AP046 | 156 — a_changed_export_binding_is_refused_before_the_store | DELETE | D5 | The Server constructs exportProjectArchive command kind as a literal. Changing it to createVolume only in the direct fixture cannot arrive from the endpoint. | apps/web/test/node-postgresql/project-export-admission-http.integration.test.ts:295 |
| AP047 | 168 — a_changed_format_profile_is_refused_before_the_store | DELETE | D5 | Server project_export.rs rejects a nonmatching archive profile before constructing the Application command. The HTTP export test owns the observable profile refusal; this duplicated inner check has no independent current input. | apps/web/test/node-postgresql/project-export-admission-http.integration.test.ts:295 |
| AP048 | 180 — get_export_operation_reports_ready_only_with_an_immutable_root | DELETE | D3 | Despite its name, the fake Reader always returns Ready and this test never removes the immutable root. It would pass if no root validation existed. Public Worker-ready download and in-progress refusal establish the actual two states. | apps/web/test/node-postgresql/project-export-admission-http.integration.test.ts:295; apps/web/test/node-postgresql/project-export-admission-http.integration.test.ts:558 |
| AP049 | 197 — verified_archive_bytes_are_refused_while_in_progress | DELETE | D4 | ProgressReader implements the Unsettled classification inside the test, and Application only forwards it. Public in-progress ZIP download requires invalid_provenance before bytes; that tests the product behavior rather than the fake Reader's own match. | apps/web/test/node-postgresql/project-export-admission-http.integration.test.ts:295 |

## crates/storyos-application/src/project_read_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP035 | 50 — stale_chapter_identity_is_refused_before_the_chapter_query | DELETE | D4 | open_current_chapter has no product call site at this baseline; only tests call it. The Adapter scope test repeats this exact stale-current helper behavior, while production Chapter access uses open_chapter. A helper-only mutation has no current product observation. | crates/storyos-adapter-postgres/tests/project_scope.rs:35; apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |

## crates/storyos-application/src/readable_export_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP050 | 142 — an_exact_export_binding_reaches_the_store | DELETE | D1 | Public readable export admits a retained operation and exact identity. The fake call counter and echoed export ID do not add a separate behavior. | apps/web/test/node-postgresql/readable-export-admission-http.integration.test.ts:233 |
| AP051 | 152 — a_changed_export_binding_is_refused_before_the_store | DELETE | D5 | Server readable_export.rs supplies exportHumanReadableManuscript as the command kind, so the fixture's createVolume tuple cannot come from a current request. | apps/web/test/node-postgresql/readable-export-admission-http.integration.test.ts:233 |
| AP052 | 164 — live_tree_order_joins_block_text_and_marks_unavailable_chapters | KEEP | K1 | A Chapter present in the canonical tree but absent from loaded payload facts must remain in readable output with the unavailable marker. Core renderer goldens start with body=None and do not test the Application join that creates it; public pinned export contains present payloads. This test uniquely checks the missing-payload join. | crates/storyos-core/src/readable_export_tests.rs:4; apps/web/test/node-postgresql/readable-export-pinned-source-http.integration.test.ts:124 |

## crates/storyos-application/src/set_current_chapter_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP011 | 75 — a_matching_challenge_reaches_the_store | DELETE | D1 | Public selection verifies the changed Current Chapter, EditorSession snapshot and canonical state. A fake Applied result and call counter add no distinct transition. | apps/web/test/node-postgresql/set-current-chapter-http.integration.test.ts:313 |
| AP012 | 86 — a_mismatched_command_kind_is_a_binding_conflict | DELETE | D5 | The Server supplies the setCurrentChapter literal with the route's computed binding. The mismatched createVolume kind is an internal-only tuple; public command binding remains protected by stored challenge validation. | apps/web/test/node-postgresql/set-current-chapter-http.integration.test.ts:313 |

## crates/storyos-application/src/takeover_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP018 | 75 — matching_bindings_reach_the_store_once | DELETE | D1 | The public takeover scenario replaces the writer and proves the old writer's late result cannot change authority. This covers the real forwarding/result path; local fake generation 2 and call-count assertions do not add a behavior. | apps/web/test/node-postgresql/takeover-late-result-http.integration.test.ts:119 |
| AP019 | 94 — canonical_payload_substitution_fails_before_the_store | DELETE | D5 | Server takeover.rs creates digest and canonical bytes from one request. Mutating only canonical bytes in a direct Application fixture is not public retry substitution; the real endpoint binds changed input against the stored challenge. | apps/web/test/node-postgresql/takeover-late-result-http.integration.test.ts:119 |

## crates/storyos-application/src/undo_latest_author_action_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP013 | 72 — a_matching_challenge_reaches_the_store | DELETE | D1 | This test only forwards a fake Unavailable result; it does not compute NoFrontier. Packaged public Undo exercises the Application dispatch and settlement path with real authority. The Core NoFrontier classifier remains a separate retained owner. Execution order: keep the cited covering test until BD021 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/browser-exact-dist/s2-undo.integration.test.ts:162; crates/storyos-core/src/undo_latest_author_action_tests.rs:59 |
| AP014 | 83 — a_mismatched_command_kind_is_a_binding_conflict | DELETE | D5 | The Server supplies UNDO_COMMAND_KIND and computes its binding before dispatch. A setCurrentChapter kind inside UndoLatestAuthorActionCommand is not accepted from the public request. Execution order: keep the cited covering test until BD021 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/browser-exact-dist/s2-undo.integration.test.ts:162 |

## crates/storyos-application/src/update_project_assistance_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP053 | 84 — an_exact_assistance_binding_reaches_the_store | DELETE | D1 | HTTP first prepare checks initialization, binding fields and revision, covering Application dispatch/result forwarding into a real Store. | apps/web/test/node-postgresql/update-project-assistance-http.integration.test.ts:134 |
| AP054 | 98 — a_changed_assistance_binding_is_refused_before_the_store | DELETE | D5 | Server supplies updateProjectAssistance from a literal. A different in-memory command kind is not accepted as request data. | apps/web/test/node-postgresql/update-project-assistance-http.integration.test.ts:134 |
| AP055 | 110 — a_changed_retry_digest_is_refused_before_the_store | DELETE | D5 | Server computes the canonical digest and bytes together. This fixture changes only bytes; real retry substitution remains a stored challenge/admission comparison at the public boundary. | apps/web/test/node-postgresql/update-project-assistance-http.integration.test.ts:134 |

## crates/storyos-application/src/update_project_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| AP003 | 72 — an_exact_update_project_binding_reaches_the_store | DELETE | D1 | The public rename case observes the real title/revision and one retained outcome; the fake Store locally constructs the same Applied result rather than testing persistence or classification. Execution order: keep the cited covering test until NP025 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:210 |
| AP004 | 86 — a_changed_update_project_binding_is_refused_before_the_store | DELETE | D5 | Server update_project.rs supplies a literal updateProject command kind. A request cannot independently replace that internal field; the public test owns real command/challenge rejection, not a mutation limited to this redundant guard. Execution order: keep the cited covering test until NP025 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:210 |
| AP005 | 98 — an_invalid_title_is_refused_before_the_store | DELETE | D5 | Server validates empty and >1024-byte titles before constructing UpdateProjectCommand. The fixture changes title alone but retains unrelated canonical bytes. It does not represent a request that reaches this Application guard; public title validation owns observable refusal. Execution order: keep the cited covering test until NP025 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:210 |
| AP006 | 116 — a_changed_retry_digest_is_refused_before_the_store | DELETE | D5 | Server computes both canonical bytes and this in-memory challenge digest from the same request. A changed HTTP retry reaches stored challenge binding validation with a new self-consistent digest; it does not create the local digest/bytes inconsistency in this fixture. Execution order: keep the cited covering test until NP025 has transferred its required assertions to its named destination. Current source coverage is not a claim that the destination already contains them. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:210 |
