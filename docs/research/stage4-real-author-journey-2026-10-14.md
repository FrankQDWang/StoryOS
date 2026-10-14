# Stage 4 real-author journey S4-JRN-001 (2026-10-14)

This record is the live evidence of [S4-05: Verify the Real-Author Model Journey](https://github.com/FrankQDWang/StoryOS/issues/396) for [Deliver Stage 4](https://github.com/FrankQDWang/StoryOS/issues/362). It records what StoryOS did on the real Agent Plan route, the live request log, the gaps found, and the claim boundary. The generated crosswalk `generated/evidence/stage4-contract-crosswalk.json` binds each S4 row to this record and to the deterministic tests.

Claim ceiling: implementation evidence completeness only (HND-003). This record does not emit EV-SR or PASS-STAGE.

## Setup

| Item | Value |
| --- | --- |
| Product code | `main` at `f03cebc0d4aa9baf0be131f686234e4907a5580c`. The release package was built at `f7a5435b`, which changes only the contract crosswalk generator. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0`, and a separate `storyos-worker` process (`--once` for each step). Both from `make release-package`. |
| Settings | ADR 0050 settings layer. `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. The key comes from the settings env file; this run never read it. |
| Database | Disposable PostgreSQL from `scripts/dev-postgres.sh`. No Chapter text was written into the database directly. Each manuscript change went through the editor. |
| Registration | Head `018f0000-0000-7001-8000-00000000fa15`, model `doubao-seed-2-1-pro-260915`, window 1,048,576, compaction threshold 812,646 (ADR 0059). |
| Author surface | The exact-dist Web client from the release package, in a browser, with the bootstrap session of the local owner. |
| Project | `01a138f4-08da-7a20-b1c6-4b83da691945` ("S4 真实作者旅程"), Chapters 雨夜 `01a138f5-005a-…` and 渡口 `01a138f5-983a-7f31-9ad3-6a41b80b4718`. |

## Live request log

The standing authorization on #362 covers each request. The planned cap started at 10. The coordinator raised it three times; each change is below.

| Cap change | Reason |
| --- | --- |
| 10 → 11 | R3 completed in 3 s, before the interrupt script saw the reference. The interrupt step needed one more Create. |
| 11 → 12 | R9 was an automatic `late_retrieval` after the successor. It is product design (S4-04c), not an anomaly. From then on, each plan counts automatic requests. |
| 12 → 13 | R11 was an automatic Retrieve by the finalizing worker after the cancellation. |
| 13 → 14 | The confirmed-expiry step needs a Conversation whose latest Attempt settled normally. Conversations 1 and 2 end in an unknown or a cancelled Attempt (see "Confirmed reference expiry"). |
| 14 → 18 | The step 5 rerun on the `main` that contains #1148: interrupted Create, Retrieve, successor, and the automatic `late_retrieval`. |
| 18 → 22 | The first rerun (S1–S4) gave no Decision, because the reply covered one of two targets. A second rerun (S5–S8) uses the two-paragraph instruction of R6. |

| # | Time (UTC) | Run | Kind | Result | Usage in / out |
| --- | --- | --- | --- | --- | --- |
| R1 | 05:51 | `01a138f6-b252-7730-bf1d-6ce5b0a0d7f6` | Create, stream, new Conversation 1 | `prose_change` on 3 blocks of 雨夜. Proposal `01a138f7-9520-78b2-acfe-2bd64b78b077`. Response `resp_02179195706213007ebaf30756c4961bf6a829967aba6508a1f2b`. Wire digest `sha256:9287ff0028f03…`. | 552 / 1048 |
| R2 | 05:53 | `01a138f9-07d3-7b71-904e-7c9c084f54bd` | Create, stream, incremental continuation of R1 | 渡口 made current; new instruction. `previous_response_id` = R1 response. `prose_change` on 2 blocks; Proposal `01a138f9-8213-76e1-a9b3-afdb0ca67e30`. | 1151 / 600 |
| R3 | 06:04 | `01a13902-b173-71e0-9075-7d1abc4a957f` | Create, continuation | Advisory. Completed before the interrupt. | 1600 / 58 |
| R4 | 06:10 | `01a13908-6beb-7053-a3c0-581b66c3faa5` | Create, stream | Worker stopped (SIGSTOP) 60 s after the reference committed, then killed. Attempt `uncertain`. | unknown |
| R5 | 06:11 | same | Retrieve by a new worker | The stored original result settled the decision (Advisory) with fence 2. No successor. | 1851 / 294 |
| R6 | 06:11:53 | `01a13909-dfbe-72c2-adc4-9807fa7239b6` | Create | Worker killed before the stream ended. Attempt `uncertain`. | unknown |
| R7 | 06:12 | same | Retrieve | Rejected: the response was not stored (404). | — |
| R8 | 06:12 | same | Successor Create | Valid `prose_change` with one change for each of the two declared targets, but the Run completed with the decision absent (gap #1148). | 2214 / 1161 |
| R9 | 06:13 | same | Automatic `late_retrieval` | Rejected. The predecessor stays unknown, fenced, allowance consumed. | — |
| — | 06:15 | `01a1390d-846e-79a1-a5c9-8d625db2b7ab` | Refused before dispatch | `execution_refused` / `expiry_rebuild_unknown_create`. No manifest, no request. | — |
| R10 | 06:26:48 | `01a13917-810c-7fc2-8f6c-6583a1bb5aea` | Create, stream, new Conversation 2 | `cancelAgentRun` through the public Host during the stream: `cancelled`, fence 2. The worker saw `stale_fence`. Attempt `uncertain`. No successor. | unknown |
| R11 | 06:27 | same | Automatic Retrieve by the finalizing worker | Rejected. The Abort was a `pre_dispatch_refusal` (the route has no cancel), so no request. | — |
| R12 | 06:31:18 | `01a1391b-9459-7af1-a170-755a5e4ca0dc` | Create, new Conversation 3 | Advisory. Response `resp_02179195947923884888bb83e57e5fbf0ee20dce84dcf97ac5b20`. | 457 / 83 |
| R13 | pending (after 14:06Z) | | Create, continuation with an expired reference | | |
| R14 | pending | | Automatic rebuild Create in the same Run | | |
| S1 | 09:26:46 | `01a139bc-494a-7953-82f5-fa8dfed5105a` | Create, stream, new Conversation 4, on `4c29bb6e` | Worker killed when the reference committed. Attempt `uncertain`. | unknown |
| S2 | 09:27:16 | same | Retrieve | Rejected: the response was not stored. | — |
| S3 | 09:27 | same | Successor Create | Valid JSON `prose_change` with one change for two declared targets. Core validation gave no decision, so no Proposal opened (see "Rerun after #1148 and #1151"). | 462 / 1160 |
| S4 | 09:28 | same | Automatic `late_retrieval` | Rejected. Run `completed`, fence 2, allowance consumed. | — |
| S5 | 09:41:57 | `01a139ca-2f70-7351-af06-1badd5177295` | Create, stream, new Conversation 5, on `4c29bb6e` | Worker killed when the reference committed. Attempt `uncertain`. | unknown |
| S6 | 09:42:27 | same | Retrieve | Rejected: the response was not stored. | — |
| S7 | 09:42 | same | Successor Create | `prose_change` with one change for each of the two targets. Decision `01a139cb-59f9-7e63-bb3d-45f3d6a29d04` opened Proposal `01a139cb-5a0a-7362-8878-265325fcfdf0`. | 459 / 1389 |
| S8 | 09:43 | same | Automatic `late_retrieval` | Rejected. Run `completed`, fence 2, allowance consumed. | — |

## Journey steps

### Step 1: complete editor

Ran on `f03cebc0`. The author created the Project, Volume 第一卷, Chapters 雨夜 and 渡口, and wrote both by hand through the editor (`applyAuthorEdit`, `createChapter`, `setCurrentChapter` Receipts, all `authoritative_applied`). Deterministic support: `s2-workspace` and `production-host` exact-dist tests.

### Step 2: bounded request and Context Inspect

Ran on `f03cebc0`. Context Inspect is a read-only audit query (ADR 0040), so this step read `storyos.context_assembly_manifests` for R2:

- `operation_requirement.purpose` = `current_passage_assistance`, `chapter_id` = 渡口, `chapter_revision_id` = `01a138f6-071b-7493-aea8-68bc5f642045`, `proposal_target_block_ids` = the two blocks of 渡口.
- `selected` = `author_instruction` (23 scalars) and `working_target` (73 scalars), both `exact_required`. `considered` also lists `host_control` and an absent `instruction_binding`. `sufficiency` = `complete`. `item_token_limit` = 32000.

The author surface showed only the result list and the summary sentence.

### Step 3: disclosure, binding, continuation, streaming, cache, usage

Ran on `f03cebc0`.

- Use binding `01a138f6-b268-7ea2-a213-e17e3a90c0eb`: Registration fa15, `credential_source` = `operator_quota`, `credential_reference` = `settings:STORYOS_AGENT_PLAN_API_KEY`, `budget_bounds` = `agent_plan_request_v2`. Grant `01a138f6-b268-7ea2-a213-e16e11405a37`, scope `project_writing_assistance_v2` (ADR 0051).
- Each dispatched Attempt has a decision manifest with both the destination context manifest and the outbound disclosure manifest. The refused Run `01a1390d-…` has neither, and it sent no request.
- R2 Attempt `01a138f9-14cc-76f1-82cd-c1637c3333ae`: `continuation.input_mapping` = `incremental`, `prior_continuation_binding_id` = R1 binding, wire `previous_response_id` = R1 response, `stream: true`, `store: true`, model `doubao-seed-2-1-pro-260915`. The serialized payload has no credential and no header.
- Evidence items separate `sent_content`, `stored_reference`, `provider_report` (`provider_usage_reported`), and `provider_opaque` (`provider_internal_content` unknown).
- Cache: `explicit` = `not_requested`, `implicit` = `destination_default`, `observed` = reported 0 cached input tokens. This proves no cache hit and no savings.
- Usage: R1, R2, R3, R5, R8, R12 have `reported` usage. R4, R6, R10 stay `unknown`.
- Instruction and target change: R2 changed the current Chapter and the instruction inside the same Conversation, and the manifest shows the new Working Target.
- Active compaction: the author waived a live compaction run on 2026-10-14. The evidence is S4-06c2b2 and S4-06c3, the fa14 qualification of #1143 (`agent-plan-compaction-threshold-qualification-2026-10-13.md`, `agent-plan-pinned-version-qualification-2026-10-14.md`), and the deterministic compaction tests in the crosswalk.

### Step 4: inspect, edit, Accept, Reject

Ran on `f03cebc0`.

- R1 Proposal: the author edited candidate 2 in the main editor (`applyAuthorEdit` → `proposal_revised`, a new Proposal revision with the edited operation), then accepted it (`acceptProposal` → `authoritative_applied`, Acceptance Receipt `authoritative_applied`). The author rejected locations 1 and 3 (`rejectProposalOperations` → `proposal_operations_resolved`, two Rejection Receipts `author_declined`).
- R2 Proposal: the author rejected both locations in a separate Run (two Rejection Receipts).
- Author Action entries 1–20 are all `forward` and match the Receipts.

### Step 5: interrupt, unknown outcome, recovery, cancellation, expiry

Ran on `f03cebc0`. Verdict on `f03cebc0`: **BLOCK** (#1148). The second rerun on `4c29bb6e` passes (see "Rerun after #1148 and #1151").

- Unknown outcome with retrieval (R4, R5): the Attempt stayed `uncertain` until a new worker retrieved the stored response. The original result settled the decision with fence 2, and no successor started.
- Unknown outcome without a stored response (R6–R9): Retrieve was rejected, the predecessor was fenced first, and one bounded successor ran (`allowance_consumed` = true, `same_request` = true, `context_changed` = false). The successor returned valid `prose_change` text, but the Run completed with no decision and no Proposal. The panel showed the approved sentence "没有收到这次的结果，正文没有改动。请开始新对话重试。". This is gap #1148: `agent_run_successor_dispatch.rs` validates the successor output against no declared targets, so `opened_proposal` is always absent.
- The automatic `late_retrieval` (R9) was rejected and kept the predecessor unknown. No second successor started.
- Spent allowance prevents another successor: the next request in Conversation 1 was refused before dispatch (`expiry_rebuild_unknown_create`), and the panel showed "上一条请求还没有结果，请开始新对话。". This is contract behavior.
- Cancellation prevents a successor: R10 was cancelled during the stream through the public Host. The worker saw a stale fence, the Attempt stayed unknown, and no successor started. The automatic Retrieve (R11) was rejected.

#### Confirmed reference expiry

Pending after 14:06Z. Plan:

- Conversation 1 ends in an `uncertain` Create (R6), so a new request gets `UnknownCreate`. Conversation 2 ends in a cancelled Run with an `uncertain` Attempt. Neither can show a confirmed expiry, so R12 opened Conversation 3 and settled normally.
- Fault injection in the disposable database: the `response_reference` of the R12 Attempt `01a1391b-b2dc-7162-9b55-e58f35ee67fd` is replaced with `resp_02179172752875438521d8aeb1173234c174f01c4d2f3c18d455c`, a real response that the same route created on 2026-10-11T14:05:28Z (`agent-plan-runtime-qualification-2026-10-11.md`, request 1). The route keeps a stored response for 72 hours.
- R13 continues Conversation 3. The expected route result is 400 `InvalidParameter.PreviousResponseNotFound`, which the adapter maps to `continuation_unusable`. `record_continuation_rejection` marks the R13 Attempt `rejected_create` and the prior reference `confirmed_unusable`.
- R14 is the automatic rebuild in the same Run (`plan_expiry_rebuild`, full Context, no `previous_response_id`). If R14 is also rejected, the Run settles and no request follows.

### Step 6: queries of Run, Proposal, disclosure, Receipt, and authority

Ran on `f03cebc0`. The queries above cover Runs, Attempts, manifests, Proposals, Receipts, and Author Actions. Also:

- Recovery row `01a1390d-99e3-79d0-ad82-d3aa6b22d692`: disposition `unknown_create`, predecessor Run R6, successor Run `01a1390d-…`.
- Project export `01a1391e-edc9-7ac0-8104-4f034b16fc7f` (status `ready`, immutable root `sha256:f96f461515d3627b2759ade1cac6c361536381916bb8e6e6ace252d239a98d54`, 76 files, 398,940 bytes) includes `model_attempts` (12 rows), `context_assembly_manifests` (12), `project_destination_grants` (1), `project_external_use_binding_revisions` (1), `processing_destination_identities` (1), and `context_reference_recoveries` (1). The only credential text is the reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No `Authorization`, `Bearer`, or key field occurs.
- On `f03cebc0`, step 6 was BLOCK together with step 5 (#1148), because the successor Run had no decision to query. The second step 5 rerun on `4c29bb6e` gives the successor decision, Proposal, and Receipts to query.

### Step 7: external model disabled

Ran on `f03cebc0`. A second `storyos-server` (port 8744, same disposable database) ran without `STORYOS_MODEL_DESTINATION`.

- The panel showed "写作助手当前不可用。你仍可以直接写作。".
- Manual writing saved (`applyAuthorEdit` → `authoritative_applied`), and Undo applied (`undoLatestAuthorAction` → `authoritative_applied`). My Return key split a paragraph in the wrong place. A second Undo returned `conflicted` / `wrong_target_head` because of that operation sequence, not because of a defect. I joined the paragraph again in the editor, and 雨夜 has its original 138 scalars.
- Writer takeover: **BLOCK** (#1151). After a writer takeover, the editor became read-only with "活动流无法同步". `takeover.rs` gives the Editor Session base snapshot one new id and persists a different new id in `project_snapshots`, so the Activity query returns 404. Work-around: "设为当前章节" on any Chapter. The takeover rerun on `6d511d93` passes (see "Step 7, takeover rerun").
- The panel sentence, manual save, and Undo results pass.
- Deterministic support: exact-dist `s2-jrn-001`, `s2-workspace`, `s2-physical-drill`, and browser-source `assistant-stopped-request`.

## Rerun after #1148 and #1151

### Step 5, first rerun (S1–S4)

Ran on `main` `4c29bb6e` (branch merge `5689ae5c`, release package rebuilt). The new Editor Session took over the writer role; "设为当前章节" on 渡口 then avoided #1151.

- The recovery path ran as planned: the predecessor was fenced, Retrieve was rejected, one successor ran (`same_request` = true, `allowance_consumed` = true), and the automatic `late_retrieval` was rejected.
- The successor reply was valid JSON, but it had one change (the second paragraph) for the two declared targets of 渡口. `prose_changes_match_targets` requires one change for each declared target, so core validation gave no decision and no Proposal. The panel showed "没有收到这次的结果，正文没有改动。请开始新对话重试。". A normal Create applies the same validation.
- The instruction asked to expand only the second paragraph. R8 had one change for each target, so #1148 did block R8. This rerun is not live evidence of #1148. The coordinator records the single-target reply problem as a separate ticket for the author; S4-05 does not change it.

### Step 5, second rerun (S5–S8): live evidence of #1148

Ran on `main` `4c29bb6e`, with the instruction of R6 ("请把这一章的两段各自扩写到大约三百字，增加渡口清晨的细节。") in a new Conversation 5.

- Recovery: the Create became `uncertain`, Retrieve was rejected, the predecessor was fenced, one successor ran, and the automatic `late_retrieval` was rejected. Run `completed`, fence 2.
- The successor decision `01a139cb-59f9-7e63-bb3d-45f3d6a29d04` (`prose_change`, `opened_proposal` present) opened Proposal `01a139cb-5a0a-7362-8878-265325fcfdf0` on 渡口. The Proposal `source_decision_id` is the successor decision.
- Editor: the panel showed "这次修改涉及 1 个章节、2 处文字", and the main editor showed both inline locations with "接受" and "拒绝". The author accepted location 1 (`acceptProposal` → `authoritative_applied`, Acceptance Receipt `authoritative_applied`) and rejected location 2 (`rejectProposalOperations` → `proposal_operations_resolved`, Rejection Receipt `author_declined`, resolution `pending` → `rejected`, Author Action 30). 渡口 has 293 scalars and shows "已保存".
- Step 5 unknown-create recovery and step 6 queries: pass on `4c29bb6e`. Confirmed reference expiry (R13, R14) is still pending.

### Step 7, takeover rerun

Ran on `main` `6d511d93` (branch merge `cee1ce88`, release package rebuilt), with no live request. A `storyos-server` without `STORYOS_MODEL_DESTINATION` ran on a new origin (port 8745). The origin had no IndexedDB database, no session storage, and no local storage.

- The new Editor Session showed "接管写作". The takeover applied (`takeOverProjectWriter`, `writer_takeover_applied`, writer generation 4).
- Each Activity query after the takeover returned 200, and the editor showed "已保存" with no "活动流无法同步".
- Manual writing saved (`applyAuthorEdit` → `authoritative_applied`), and Undo restored the text (`undoLatestAuthorAction` → `authoritative_applied`).
- The panel showed "写作助手当前不可用。你仍可以直接写作。".
- Step 7: pass on `6d511d93`.

## Observations of #396

- A malformed JSON reply becomes an Advisory. This is contract behavior; the UX follow-up is #1145. Not blocking.
- Outline selection is read-only by Stage 2 design; the UX follow-up is #1146. Not blocking. The journey uses "设为当前章节" to change the current Chapter.

## Gaps

| Issue | Finding | Effect on S4-JRN-001 |
| --- | --- | --- |
| #1145 | Malformed reply UX | None |
| #1146 | Outline selection is read-only | None |
| #1147 | A stale candidate target of a rejected Proposal makes the next send fail with "无法提交请求，请稍后再试。". Work-around: click ordinary prose to clear the focus, then send. | None with the work-around |
| #1148 | The successor result has no decision | Fixed by PR #1150; the second step 5 rerun on `4c29bb6e` passes |
| #1161 | A reply that changes only some declared targets gives no Proposal | One live request after #1161 merges |
| #1151 | Writer takeover leaves the base snapshot id out of `project_snapshots`, so the editor becomes read-only | Fixed by PR #1154 (ADR 0060); the takeover rerun on `6d511d93` passes |

## Superseded contract text

The S4 user stories that mention a Keychain credential store follow ADR 0050: the Keychain path was removed in #1099, and the credential reference is `settings:STORYOS_AGENT_PLAN_API_KEY`.

## Claim boundary (S4-REQ-007, S4-EVD-006)

StoryOS claims only its own disclosure and recovery evidence. These stay unclaimed:

- Provider attention to any input item, and how the model used the stored previous response.
- Provider retention and training, other than the documented 72-hour stored-response window that this run used as a fault source.
- Hidden SDK or gateway behavior, and the implicit cache of the destination (reported 0 cached tokens proves no hit).
- Model quality and literary quality of each reply.
