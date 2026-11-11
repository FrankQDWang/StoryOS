# Stage 5 governed research journey S5-JRN-001 (2026-11-11)

This record is the live evidence of [S5-09: Verify the Governed Research Journey](https://github.com/FrankQDWang/StoryOS/issues/405) for [Deliver Stage 5](https://github.com/FrankQDWang/StoryOS/issues/363). It records what StoryOS did on the real Agent Plan route and the real Exa MCP server, the live request log, each journey step, the requirement and evidence crosswalk, the gaps, and the claim boundary.

Claim ceiling: implementation evidence completeness only (HND-003). This record does not emit HND-004, EV-SR, or PASS-STAGE. The parent #363 evaluates them.

## Setup

| Item | Value |
| --- | --- |
| Product code | `main` at `74d0ff90cfb5daecff41ba1337b202bceb47dfb0` (tree `b8140b8c`). The branch of this ticket changes no product file. |
| Release package | `make release-package` on the clean `main` above, manifest `sha256:56412b1ccc816d4fe41138604393dcd8f7fd7d05e46d402f2601d42632333f79`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` on `127.0.0.1:8746`, and a separate `storyos-worker --once` for each step. Both from the release package. |
| Settings | ADR 0050 settings layer. Live steps: `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`; the Worker resolved both keys from the settings env file of the main checkout. This run never read, copied, or printed that file or a key. Fake steps: `STORYOS_MODEL_DESTINATION=host_fake` with test-only keys in the process environment, so the Worker read no env file and sent no real key to a fake. |
| Database | Disposable PostgreSQL from `scripts/dev-postgres.sh up --interactive`. No Chapter text, Source, ToolCall, or Artifact was written into the database directly. Each manuscript change went through the editor. The only direct writes were the guard schema `live_guard`, the destination endpoints of Project B (as the Postgres tests and the exact-dist journey do), and the list of fake Projects of the guard. |
| Request guard | A harness of the disposable database, not a product change: two triggers count each claim of Project A. The insert of a Model Attempt counts one Agent Plan request. The first `destination_attempt_id` of a ToolCall counts one Exa exchange. Above the cap (14 Agent Plan, 11 Exa, 25 in all) the trigger refuses the claim transaction, so nothing goes out. Fake Projects are not counted. |
| Author surface | The exact-dist Web client that the Server serves, in the built-in browser of the Claude desktop app, with the bootstrap session of the local owner. The site storage of the origin was clear before step 1. |
| Project A (live) | `01a1c8ea-02ee-7eb0-aaa1-4fbbda420cbc` ("S5 研究旅程"), Chapter 潮水 `01a1c8ea-81b3-7600-ab1a-975d853995f6`. |
| Project B (fake) | `01a1c8eb-5993-7fd2-b81a-efbedc07ac45` ("S5 假服务步骤"), Chapter 假章 `01a1c8ec-eddf-7961-b873-9579399b7574`. Its destination evidence points at the fake Agent Plan route and the fake Exa MCP server of the exact-dist journey (`apps/web/test/support/research-journey-fakes.ts`). |

## Live request log

Authorization: the author authorized the live plan of [#405 comment 6787170283](https://github.com/FrankQDWang/StoryOS/issues/405#issuecomment-6787170283) on 2026-11-11 ("授权运行"), relayed by the coordinator session: 6 Agent Plan requests and 3 Exa exchanges, hard cap 25 in all, counted at the claim. The request log on #405 starts at [comment 6787342212](https://github.com/FrankQDWang/StoryOS/issues/405#issuecomment-6787342212).

The planned count did not change. The final count is **6 Agent Plan requests and 3 Exa exchanges, 9 of 25**. The guard counted each one at its claim. No automatic Retrieve occurred.

| # | Claim (UTC) | Row | Kind | Result | Usage in / out |
| --- | --- | --- | --- | --- | --- |
| R1 | 04:50:59 | Attempt `01a1c8f1-e81b-7a30-a181-02f15da872c0` | Create, stream, Run `01a1c8f1-be6a-7ac1-a769-cda0e8d2b703` | `function_call` `web_search` (`call_9mssrrqe1b6tdqrtwq3vqg9g`). Response `resp_0217943726593965c7f85291e6ad850e47f0b8bb356de2f32cf63`. | 1351 / 143 |
| E1 | 04:51:03 | ToolCall `01a1c8f1-f994-7c42-b174-d46d36d27b0b` | `web_search_exa` | `settled`; `external_read` and `outbound_disclosure` `confirmed`; 26,944 result bytes. | — |
| R2 | 04:51:09 | Attempt `01a1c8f2-0e9a-7ee0-9bca-d66700a02620` | Continuation | `function_call_output` of `call_9mss…`, then `function_call` `web_fetch` (`call_jor641h3aubhyh970h953zaz`) of one page and the `.invalid` URL. | 9250 / 164 |
| E2 | 04:51:15 | ToolCall `01a1c8f2-2697-76d0-8f64-48ad49bcff62` | `web_fetch_exa` | `settled`; `confirmed`; 3,157 result bytes; one Source Gap. | — |
| R3 | 04:51:19 | Attempt `01a1c8f2-3696-7241-b752-c37b52fc61d4` | Continuation | `function_call_output` of `call_jor…`, then `research_synthesis`. Streamed for 5 min 50 s. | 10405 / 13473 |
| R4 | 04:58:20 | Attempt `01a1c8f8-a4fb-78c1-b267-3277d800a727` | Continuation, Run `01a1c8f8-8387-7db1-a5e6-197c29057ca6` ("用到故事里") | `prose_change`; Proposal `01a1c8f9-49a0-7e72-a492-c0faa110e6f6`. | 12079 / 1313 |
| R5 | 05:00:34 | Attempt `01a1c8fa-aefd-71c0-b0fc-43f4e09fe178` | Create, new Conversation, Run `01a1c8fa-8f33-7973-8f5d-488905835fc4` | Two `web_search` calls in one batch. | 1302 / 276 |
| E3 | 05:00:42 | ToolCall `01a1c8fa-d01c-7461-acf1-52273f7502a1` | `web_search_exa` | Worker stopped during the exchange; Run cancelled; `unknown`. | — |
| R6 | 05:03:15 | Attempt `01a1c8fd-2418-7ea1-b569-29acbc81e062` | Run `01a1c8fd-1aff-78f3-90f8-59b2815a58de`, research disabled | No `tools` field; `prose_change`; Proposal `01a1c8fd-4e9d-7b00-83e2-a493c417905f`. | 577 / 364 |

The model was `doubao-seed-2-1-pro-260915` (model Registration `018f0000-0000-7001-8000-00000000fa17`). The Tool Registration revision was `018f0000-0000-7001-8000-0000000e0001` (`https://mcp.exa.ai/mcp`, protocol `2025-11-25`) until step 7.

Key check: the Worker logs of each live step, the Server log, a data dump of the `storyos` schema, and the Project Export archive have no `Authorization` header, no bearer token, and no `x-api-key`. The one match of "authorization" in the archive is the column name `qualification_authorization`. The database names each key only by its Credential Reference (`settings:STORYOS_AGENT_PLAN_API_KEY`, `settings:STORYOS_EXA_API_KEY`).

## Journey steps

| Step | S5-JRN-001 item | Identifiers | Observed outcome |
| --- | --- | --- | --- |
| 1 | Manual writing and enablement | Project A, Chapter 潮水 | The author typed two paragraphs in the editor and enabled web research in the panel (`setProjectToolEnablement`). No prompt. No request. |
| 2 | Research question; search and fetch decided by the model with no prompt; native correlation; sources and a gap; Research Artifact with citations | Run `01a1c8f1-be6a-…`, R1 to R3, E1 and E2, Artifact `01a1c8f7-8b0b-7162-947b-dbc254e1194e`, Gap `01a1c8f2-3657-7da2-a731-8d94471b0d35` | The model chose `web_search`, then `web_fetch` of `https://www.liverpool.ac.uk/~cmi/mersey/liv1870.html` and `https://harbour-archive.invalid/liverpool-1890`. Each Exa exchange was admitted under grant scope `research_tool_v1` with no prompt. The `function_call` items of R1 and R2 and the `function_call_output` items of R2 and R3 have the same `call_id` (`call_9mss…`, `call_jor…`); each continuation sent `previous_response_id`. The panel showed 11 sources, the gap line `https://harbour-archive.invalid/liverpool-1890：无法获取`, 11 claims with numbered citations, and the citation list with `[10] …：无法获取（读取失败）`. The Artifact has 11 Source Snapshots and 1 Source Gap (`failed`). |
| 3 | Prose Proposal and Rejection without loss of sources | Run `01a1c8f8-8387-…`, R4, Proposal `01a1c8f9-49a0-…` | "用到故事里" sent `请根据上面的研究结果修改当前章节的正文。` in the same Conversation. The Proposal has 2 operations and `source_lineage` `research_artifact` `01a1c8f7-8b0b-…` with the producing Attempt. The panel said "这次修改涉及 1 个章节、2 处文字". The author rejected both operations in the editor. The Chapter stayed unchanged. The digests of the Artifact, the 11 Snapshots, and the Gap rows are the same before and after the Rejection. |
| 4 | Cancellation during a dispatch, with its outcome class | Run `01a1c8fa-8f33-…`, R5, E3, fence `01a1c8fc-1c9d-7852-8682-f8ebd7561262` | The Worker was stopped 0.08 s after the E3 claim. The author pressed "暂停" (Run `paused`); the executor then cancelled the Run through the public `cancelAgentRun` (see "Deviations"). E3 settled `unknown`, class `unknown`: `external_read` and `outbound_disclosure` `unknown`, fence cause `run_cancelled`, `local_stop` `drop_exchange`, `remote_abort` `unsupported`, Destination Attempt `outcome_unknown`. The second search ToolCall `01a1c8fa-d01f-7122-a355-f452a1b2e83a` was `refused` `run_cancelled`, class `not_attempted`, with no exchange. After the resume, the Worker saw `stale_fence` and dropped the exchange. Panel: "可能已联网搜索，但没有收到结果：…" and "有一次联网检索没有完成。". |
| 5 | Disablement and manual writing | Run `01a1c8fd-1aff-…`, R6 | The author disabled web research. R6 has no `tools` field and no ToolCall. The author typed "雾散了。" (saved) and pressed Undo (`undoLatestAuthorAction`); after a reload the Chapter has the two original paragraphs. |
| 6 | One complete batch refusal (fake adapter) | Project B, Run `01a1c8ee-1665-7b40-a808-45adbc08257f`, Attempt `01a1c8ee-8005-71b3-b9a4-6d1a39ceaad1` | The fake model sent a valid `web_search` call and a `web_fetch` call with `{"urls":["not a url"]}`. The Attempt keeps `tool_call_batch` `{"kind":"invalid","refusal":"intake_url"}` and its 2 items. No ToolCall exists. The fake MCP server got no request. Run `refused` with `research_tool_unavailable`. |
| 7 | Drift quarantine (fake server) | Project B, Run `01a1c8fe-d6b7-71f1-afc5-70b8f799c3a2`, ToolCall `01a1c8fe-e02d-7cb1-bca7-10526a453e56` | The fake server listed `web_search_exa` with one more required argument. Before the call, the drift check quarantined Tool Registration revision `018f0000-…-0000000e0001` with the difference `input_schema` of `web_search_exa` (pinned `sha256:0653bc29…`, observed `sha256:688da1db…`), and Run Event `tool_registration_quarantined`. The ToolCall was `refused` `tool_registration_unavailable`, Tool Effect Outcome `not_attempted`. The fake server got `initialize`, `notifications/initialized`, and `tools/list`, and no `tools/call`. The continuation had no `tools` field. The panel shows "网络研究暂时不可用。". |
| — | Project Export | Export `01a1c8ff-fa75-7361-b166-0011ddeff474` of Project A | The archive (74 files) has `canonical/tool_calls.json` 4 rows, `source_snapshots.json` 11, `source_gaps.json` 1, `research_artifacts.json` 1, and `project_tool_enablements.json` 2. Each count is equal to the database. |

### Deviations

- Step 4: the author panel has no Run Cancellation control. Its stop button is "暂停" (`pauseAgentRun`). The executor pressed it during the dispatch, then cancelled the Run through the public `cancelAgentRun` command of the same Server, as step 5 of the Stage 4 journey did (Receipt `01a1c8fc-1c91-71d1-80b3-d9278f049c99`). It made no live request.
- Step 7: the fake model had one scripted reply. After the drift refusal the continuation got HTTP 500 from the fake, so Run `01a1c8fe-…` is `waiting` with `outcome_unknown`. This is a harness limit; the drift facts above do not depend on it.
- The step 2 message named the `.invalid` URL so that a gap would occur. The model put it into its `web_fetch` call itself.
- The Project Export used the public `exportProjectArchive` command and a Worker with `host_fake`, because the author panel has no archive export control. It made no live request.

## Exact-dist journey

`apps/web/test/browser-exact-dist/research-journey.integration.test.ts` drives the same path through the production Web client, Server, and Worker with the fake Agent Plan route and the fake Exa MCP server. The Server runs with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, because `host_fake` is not qualified for function calling and exposes no Tool. The journey then points the destination evidence of its Project at the fakes, as the Postgres tests do. The Worker runs with `host_fake` and test-only keys. The journey checks:

1. The author writes two paragraphs and enables web research in the panel. No prompt follows.
2. One research question. The fake model asks for `web_search`, then `web_fetch` of one page and one missing URL. Each continuation sends the result as `function_call_output` with the original `call_id`. The fake server gets `web_search_exa` and `web_fetch_exa` once each.
3. The panel shows the three sources, the gap of the missing URL, the two claims with numbered citations, and the citation list.
4. "用到故事里" opens a prose Proposal in the same Conversation with `source_lineage` of the Research Artifact. The author rejects it. The Artifact and the sources stay unchanged.
5. The author disables web research. The next request has no `tools` field and no ToolCall. Manual writing saves.
6. The Project Receipts are exactly the author actions of the journey.

The physical recovery drill (`scripts/verify-recovery-hold.sh`, in `make check-full`) now runs this journey on the primary database before the backup. It records the count and a digest of each Stage 5 record family (ToolCalls, Source Snapshots, Source Gaps, Research Artifacts, Project tool enablements, Tool Registration revisions), refuses an empty family, and compares them after the restore.

## Requirement and evidence crosswalk

Each row names the delivered child evidence on `main` `74d0ff90`. Each named test is in the `make check` set (Rust and Postgres tests, browser-source tests) or the `make check-full` set (exact-dist journeys and the drill) of the head of this ticket, and that head changes no product file. Thus the evidence is current, not replayed from an earlier tree. Each PASS below holds only together with the `make check` and `make check-full` Check Evidence Lines of the pull request head; a failed or missing line blocks the row.

| Row | Delivered evidence | Journey evidence (this record) | Result |
| --- | --- | --- | --- |
| `S5-REQ-001` | S5-01 #397: Registration, Enablement, and Exposure stay distinct (`tool_registration.rs`, `project_tool_enablement.rs`, `an_active_compatible_enablement_on_a_qualified_registration_exposes_its_tools`). S5-03 #399: the one Tool Gateway (`tool_gateway.rs`, `a_claimed_search_settles_with_its_result_text_and_one_run_event`), the effect intersection (`both_research_effects_dispatch_and_each_changed_authority_refuses_with_its_reason`). Hosted clauses waived (ADR 0084, #363). | Steps 2 to 5 | PASS |
| `S5-REQ-002` | S5-01 #397: the exact contract and the drift quarantine (`a_changed_identity_schema_or_removed_approved_tool_is_drift_that_names_the_difference`, `drift_quarantines_one_revision_once_and_a_new_revision_carries_no_authority_from_it`), the Credential Reference only (`diagnostics_carry_no_key_content_or_session_identifier`). S5-03 #399: the drift check before each call (`tool_contract_drift_quarantines_the_revision_and_sends_no_call`), the recheck at the claim (`each_authority_change_before_the_claim_refuses_with_its_reason_and_sends_nothing`). | Step 7 (fake), key check | PASS |
| `S5-REQ-003` | S5-02 #398: the complete batch (`one_invalid_member_rejects_the_whole_batch`, `a_durable_function_request_decision_derives_one_tool_call_once`). S5-05 #401: `function_call_output` with the original `call_id` (`a_continuation_sends_each_output_with_its_call_id_after_the_original_calls`), Context Assembly re-entry (`each_resolved_tool_call_is_one_candidate_named_by_its_snapshots_with_its_citable_ids`), results stay data (`an_imperative_result_stays_data_and_writes_no_chapter_and_no_proposal`). S5-10 #696: `function_calling` qualified (`agent-plan-function-calling-probe-2026-11-03.md`). Hosted clauses waived. | Steps 2, 6 (fake) | PASS |
| `S5-REQ-004` | S5-06 #402: Snapshots and Gaps with byte spans (`a_settled_search_captures_one_snapshot_per_entry_that_its_continuation_names`, `a_fetch_records_a_failed_a_missing_and_an_empty_url_as_gaps_and_never_as_snapshots`), the panel (`assistant-research-sources`). S5-07 #403: the Research Artifact (`a_valid_synthesis_becomes_one_immutable_artifact_with_its_lineage_and_citations`, `a_synthesis_with_an_uncited_claim_an_uncaptured_citation_or_an_unstorable_text_is_refused`, `a_synthesis_that_cites_only_gaps_is_a_partial_artifact`), the panel (`assistant-research-synthesis`). S5-09: exact-dist `research-journey`. | Step 2 | PASS |
| `S5-REQ-005` | S5-08 #404: the fence and the four classes (`tool_call_recovery_tests`, `a_cancel_during_the_exchange_drops_it_locally_and_keeps_the_tool_call_unknown`, `a_result_after_the_cancellation_fence_is_kept_as_evidence_only`, `a_takeover_after_the_claim_settles_unknown_at_the_next_claim_with_no_successor`), the worst-case reservation. S5-01 #397: the typed intakes (`a_search_intake_takes_one_bounded_query_and_an_optional_count_of_at_most_ten`, `a_fetch_intake_takes_at_most_five_absolute_web_urls`). S5-03 #399: the 256 KiB result bound and the 60 s exchange bound. Hosted clauses waived. | Step 4 | PASS |
| `S5-REQ-006` | S5-05 #401: a prose Proposal through the producer path with lineage (`a_prose_continuation_opens_a_proposal_with_its_tool_result_lineage`). S5-07 #403: "用到故事里" (`use_in_the_story_opens_a_proposal_with_the_artifact_and_a_rejection_keeps_it`). S5-09: exact-dist `research-journey` (Rejection with the Artifact and the sources unchanged). | Step 3 | PASS |
| `S5-EVD-001` | S5-10 #696 qualification; S5-01 #397 registration and discovery (`exa-mcp-discovery-2026-11-03.md`); S5-03 #399 first governed calls (`exa-first-governed-call-2026-11-10.md`), grant `research_tool_v1`, refusals before dispatch. No Approval target exists (S5-04 #400 closed, decision 11 of #363). | Steps 2, 4, 6, 7 | PASS |
| `S5-EVD-002` | S5-02 #398, S5-05 #401 (`exa-result-reentry-2026-11-10.md`), S5-06 #402, S5-07 #403 tests above; the `provider_report` and `provider_opaque` evidence items of each Model Attempt. | Steps 2, 3, 6 | PASS |
| `S5-EVD-003` | S5-08 #404 tests above; S5-07 #403 Proposal-only change; the Stage 2 AI-disabled journeys (`s2-jrn-001`, `s2-workspace`, and the other `s2-*` exact-dist journeys) and the physical drill `s2-physical-drill` and `scripts/verify-recovery-hold.sh`, which now carry the Stage 5 record families, in `make check-full` of this head. | Steps 4, 5 | PASS |
| `S5-JRN-001` | This record and exact-dist `research-journey`. | Steps 1 to 7 | see "Journey steps" |

## Decisions

- No generated Stage 5 crosswalk under `generated/evidence/`. The cleanup removed the generated crosswalks and their generator (`be48e2f3`). This record holds the crosswalk table, and the named tests are in `make check` or `make check-full` of the head of this ticket.
- No change to a global exact-dist Receipt list. The cleanup removed that list (`93ef44ab`). The research journey asserts its own Project Receipts.
- The tool-call steps of the exact-dist journey use the real Agent Plan adapter against a local fake route. The `host_fake` destination is not qualified for function calling, and a product change only for a test path is out of scope.
- The batch refusal (step 6) and the drift quarantine (step 7) run with the fakes. A live model cannot be made to send an invalid batch, and the live Exa server cannot be made to change its contract. The drift step is last, because a quarantine applies to the Tool Registration revision of each Project of the database.

## Gaps

No finding blocks S5-JRN-001. Each item below is a proposal for the parent #363 to triage.

| Finding | Effect on S5-JRN-001 | Owner |
| --- | --- | --- |
| The author panel has no Run Cancellation control; only "暂停". A cancellation needs the public command. | None: step 4 used the public `cancelAgentRun`, as in Stage 4. | Proposed new UX ticket under #363. |
| After an invalid batch with web research enabled, the panel says "模型想要联网检索，但检索功能还没有开放". The sentence dates from before tool execution, and it is wrong now: the cause was an invalid call. | None. | Proposed new UX ticket under #363 (the sentence of `research_tool_unavailable`). |
| In the panel, an earlier research turn shows only "研究结果如下。…" and no sources or claims, because the panel shows them for the current Run only. The Artifact stays readable through `getResearchArtifact`. | None. | Observation for S5-07 (#403, closed); proposed UX follow-up. |
| One URL can be two Sources: a search entry and the fetched page. R3 cited both (`[5][6]`). | None; each capture is its own Snapshot by design. | Observation only. |
| R3 streamed for 5 min 50 s with 13,473 output tokens. | None; inside the 600 s request bound and the output bound. | Observation only. |

## Claim boundary (HND-003)

StoryOS claims only its own tool governance, disclosure, and recovery evidence. These stay unclaimed:

- What the provider or Exa did with each request after it left StoryOS, and how the model used each tool result.
- Provider and Exa retention and training.
- Hidden SDK, gateway, or MCP server behavior that StoryOS cannot observe.
- The truth of each web page, and the model quality and literary quality of each reply.

This record states HND-003 (implementation evidence completeness) only. HND-004, EV-SR, and PASS-STAGE belong to the parent #363.
