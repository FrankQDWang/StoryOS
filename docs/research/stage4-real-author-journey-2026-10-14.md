# Stage 4 real-author journey S4-JRN-001 (2026-10-14)

This record is the live evidence of [S4-05: Verify the Real-Author Model Journey](https://github.com/FrankQDWang/StoryOS/issues/396) for [Deliver Stage 4](https://github.com/FrankQDWang/StoryOS/issues/362). It records what StoryOS did on the real Agent Plan route, the live request log, the gaps found, and the claim boundary. The generated crosswalk `generated/evidence/stage4-contract-crosswalk.json` binds each S4 row to this record and to the deterministic tests.

Claim ceiling: implementation evidence completeness only (HND-003). This record does not emit EV-SR or PASS-STAGE.

## Setup

| Item | Value |
| --- | --- |
| Product code | `main` at `f03cebc0d4aa9baf0be131f686234e4907a5580c`. The release package was built at `f7a5435b`, which changes only the contract crosswalk generator. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0`, and a separate `storyos-worker` process (`--once` for each step). Both from `make release-package`. |
| Settings | ADR 0050 settings layer. `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. The key comes from the settings env file; this run never read it. |
| Database | Disposable PostgreSQL from `scripts/dev-postgres.sh`. No Chapter text was written into the database directly. Each manuscript change went through the editor. The rerun after #1165 used a second disposable database (see "Rerun after #1165"). |
| Registration | Head `018f0000-0000-7001-8000-00000000fa15`, model `doubao-seed-2-1-pro-260915`, window 1,048,576, compaction threshold 812,646 (ADR 0059). |
| Author surface | The exact-dist Web client from the release package, in a browser, with the bootstrap session of the local owner. |
| Project | `01a138f4-08da-7a20-b1c6-4b83da691945` ("S4 真实作者旅程"), Chapters 雨夜 `01a138f5-005a-…` and 渡口 `01a138f5-983a-7f31-9ad3-6a41b80b4718`. |

## Live request log

The standing authorization on #362 covers each request. The planned cap started at 10. The coordinator raised it seven times; each change is below. The final count is 25, equal to the cap.

| Cap change | Reason |
| --- | --- |
| 10 → 11 | R3 completed in 3 s, before the interrupt script saw the reference. The interrupt step needed one more Create. |
| 11 → 12 | R9 was an automatic `late_retrieval` after the successor. It is product design (S4-04c), not an anomaly. From then on, each plan counts automatic requests. |
| 12 → 13 | R11 was an automatic Retrieve by the finalizing worker after the cancellation. |
| 13 → 14 | The confirmed-expiry step needs a Conversation whose latest Attempt settled normally. Conversations 1 and 2 end in an unknown or a cancelled Attempt (see "Confirmed reference expiry"). |
| 14 → 18 | The step 5 rerun on the `main` that contains #1148: interrupted Create, Retrieve, successor, and the automatic `late_retrieval`. |
| 18 → 22 | The first rerun (S1–S4) gave no Decision, because the reply covered one of two targets. A second rerun (S5–S8) uses the two-paragraph instruction of R6. |
| 22 → 25 | The step 2 and step 4 rerun with the selection references of #1165, which also gives the part-only request of #1161. Three Creates; no automatic request was expected or sent. |

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
| R13 | 10-15 02:20:14 | `01a13d5c-1b8f-75e2-8638-0f92c3a0d561` | Create, continuation of Conversation 3 with the injected expired reference | 400 `InvalidParameter.PreviousResponseNotFound`. Attempt `01a13d5c-3195-71f1-864a-54ffd882d868` `rejected_create`. | unknown |
| R14 | 10-15 02:20 | same | Automatic rebuild Create in the same Run | Full Context, no `previous_response_id`. Advisory. Response `resp_021792030821236b6ef94156a979b704a6b85dbeb7998fa8e7b1d`. Run `completed`. | 805 / 125 |
| S1 | 09:26:46 | `01a139bc-494a-7953-82f5-fa8dfed5105a` | Create, stream, new Conversation 4, on `4c29bb6e` | Worker killed when the reference committed. Attempt `uncertain`. | unknown |
| S2 | 09:27:16 | same | Retrieve | Rejected: the response was not stored. | — |
| S3 | 09:27 | same | Successor Create | Valid JSON `prose_change` with one change for two declared targets. Core validation gave no decision, so no Proposal opened (see "Rerun after #1148 and #1151"). | 462 / 1160 |
| S4 | 09:28 | same | Automatic `late_retrieval` | Rejected. Run `completed`, fence 2, allowance consumed. | — |
| S5 | 09:41:57 | `01a139ca-2f70-7351-af06-1badd5177295` | Create, stream, new Conversation 5, on `4c29bb6e` | Worker killed when the reference committed. Attempt `uncertain`. | unknown |
| S6 | 09:42:27 | same | Retrieve | Rejected: the response was not stored. | — |
| S7 | 09:42 | same | Successor Create | `prose_change` with one change for each of the two targets. Decision `01a139cb-59f9-7e63-bb3d-45f3d6a29d04` opened Proposal `01a139cb-5a0a-7362-8878-265325fcfdf0`. | 459 / 1389 |
| S8 | 09:43 | same | Automatic `late_retrieval` | Rejected. Run `completed`, fence 2, allowance consumed. | — |
| R15 | 10-15 19:34 | `01a1410e-8c7e-7a01-be41-35114da2abf2` | Create, new Conversation (second database), on `bda9138a` | One `chapter_text` reference to part of paragraph 2. `prose_change` on 1 of 2 declared blocks; Proposal `01a1410e-d449-7e70-99b9-f28d84972647`. Response `resp_021792092844693cd31eac8229c74daa7d4d74052dc3c321080d4`. | 741 / 349 |
| R16 | 10-15 19:36 | `01a14110-7e95-71e0-9445-acb74a89f7b0` | Create, incremental continuation of R15 | One `chapter_text` reference to both paragraphs. `prose_change` on 2 blocks; Proposal `01a14110-e0f6-7731-884a-521cd368f368`. | 1454 / 733 |
| R17 | 10-15 19:37 | `01a14111-6e3b-73e3-9dd8-9e184d3a8027` | Create, incremental continuation of R16 | One `proposal_candidate_text` reference to R16 location 1. Candidate revision, outcome `revised`. | 2013 / 978 |

## Journey steps

### Step 1: complete editor

Ran on `f03cebc0`. The author created the Project, Volume 第一卷, Chapters 雨夜 and 渡口, and wrote both by hand through the editor (`applyAuthorEdit`, `createChapter`, `setCurrentChapter` Receipts, all `authoritative_applied`). Deterministic support: `s2-workspace` and `production-host` exact-dist tests.

### Step 2: bounded request and Context Inspect

Ran on `f03cebc0`. The Working Target part is superseded by the rerun with selection references on `bda9138a` (see "Rerun after #1165"); this Context Inspect evidence stays valid. Context Inspect is a read-only audit query (ADR 0040), so this step read `storyos.context_assembly_manifests` for R2:

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
- Usage: R1, R2, R3, R5, R8, R12, R14, R15, R16, R17 have `reported` usage. R4, R6, R10 stay `unknown`. The rejected R13 has `unknown` usage.
- Instruction and target change: R2 changed the current Chapter and the instruction inside the same Conversation, and the manifest shows the new Working Target.
- Active compaction: the author waived a live compaction run on 2026-10-14. The evidence is S4-06c2b2 and S4-06c3, the fa14 qualification of #1143 (`agent-plan-compaction-threshold-qualification-2026-10-13.md`, `agent-plan-pinned-version-qualification-2026-10-14.md`), and the deterministic compaction tests in the crosswalk.

### Step 4: inspect, edit, Accept, Reject

Ran on `f03cebc0`. Superseded by the rerun with selection references on `bda9138a` (see "Rerun after #1165"). #1165 removed the caret-based candidate target.

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

Ran on 2026-10-15 on the release package of branch head `7358c9c5` (`main` `020e5575`). Verdict: **pass**.

- Why Conversation 3: Conversation 1 ends in an `uncertain` Create (R6), so a new request gets `UnknownCreate`. Conversation 2 ends in a cancelled Run with an `uncertain` Attempt. Neither can show a confirmed expiry, so R12 opened Conversation 3 and settled normally.
- Fault injection in the disposable database at 2026-10-15T02:19:54Z: the `response_reference` of the R12 Attempt `01a1391b-b2dc-7162-9b55-e58f35ee67fd` changed from `resp_02179195947923884888bb83e57e5fbf0ee20dce84dcf97ac5b20` to `resp_02179172752875438521d8aeb1173234c174f01c4d2f3c18d455c`. That is a real response that the same route created on 2026-10-11T14:05:28Z (`agent-plan-runtime-qualification-2026-10-11.md`, request 1). The route keeps a stored response for 72 hours, so it expired at 2026-10-14T14:05:28Z. `binding_reference` in `agent_run_create_dispatch.rs` reads this payload field as the `previous_response_id`.
- Why not the panel: the writing assistant panel continues only the Conversation that it keeps in the session storage of its tab, or starts a new one. Its current Conversation was 5. So a script sent R13 with the same public command as the panel: `createProjectCommandChallenge`, then `createAgentRun` from the generated release-1 client, `POST /api/v1/projects/{project_id}/agent-runs`, `command_schema` `storyos.command.create-agent-run.request.v2`, `conversation` = `{ kind: "existing", conversation_id: "01a1391b-9459-7af1-a170-754e73e08ae8" }`, `working_target` = `current_chapter` 渡口, `instruction` absent, `cause` `author_request`, author message "不要修改正文。请用一句话告诉我：这一章发生在一天中的什么时候？". The script sent only the bootstrap session cookie and the anti-forgery nonce; it had no credential. One `storyos-worker --once` then ran the Run.
- R13: the continuation sent the expired reference as `previous_response_id`. The route returned 400 `InvalidParameter.PreviousResponseNotFound`. The R13 Attempt became `rejected_create`, `settled`, with `rejection` = `continuation_unusable:http_400:InvalidParameter.PreviousResponseNotFound`. The R12 Attempt now has `produced_binding.reference_condition` = `confirmed_unusable`.
- R14: in the same worker pass and the same Run, `plan_expiry_rebuild` admitted one rebuild Create. Its wire payload has no `previous_response_id` and no prior continuation binding, and it carries the full Context. The recovery row `01a13d5c-3ebf-74c3-b206-650e2bf2833b` has disposition `rebuilt`, predecessor Run R12, successor Run R13/R14, `opaque_reused` = false, `covered_content_included` = true. R14 settled as decision `01a13d5c-6331-72e3-8629-dbabfb3f8cda` (Advisory "这一章发生在清晨天刚蒙蒙亮、朝阳还未升高的时候。", `advances_continuation` = true). The Run completed. No more requests followed.

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
- Step 5 unknown-create recovery and step 6 queries: pass on `4c29bb6e`. Confirmed reference expiry (R13, R14): pass on `020e5575` (see "Confirmed reference expiry").

### Step 7, takeover rerun

Ran on `main` `6d511d93` (branch merge `cee1ce88`, release package rebuilt), with no live request. A `storyos-server` without `STORYOS_MODEL_DESTINATION` ran on a new origin (port 8745). The origin had no IndexedDB database, no session storage, and no local storage.

- The new Editor Session showed "接管写作". The takeover applied (`takeOverProjectWriter`, `writer_takeover_applied`, writer generation 4).
- Each Activity query after the takeover returned 200, and the editor showed "已保存" with no "活动流无法同步".
- Manual writing saved (`applyAuthorEdit` → `authoritative_applied`), and Undo restored the text (`undoLatestAuthorAction` → `authoritative_applied`).
- The panel showed "写作助手当前不可用。你仍可以直接写作。".
- Step 7: pass on `6d511d93`.

## Rerun after #1165

#1165 (S4-08, ADR 0062) was delivered by PR #1168, #1170, #1171, #1172, #1173, and #1174 (`main` `bda9138a`). The author decided that steps 2 and 4 run again with the new interaction, and that the part-only request of #1161 (ADR 0061, PR #1162) joins this rerun.

### Setup of the rerun

- Branch merge `6605d07a` of `main` `bda9138a`, release package rebuilt.
- `main` `bda9138a` adds migration 0093, so the persistence identity of the package changed. The packaged `storyos-storage` refused the first disposable database with `identity_mismatch`; release 1 has no in-place upgrade. A second disposable database (container `storyos-s405-journey2`) was activated by the new package and loaded with the controlled fixture. The first database stays unchanged for the R1–R14 queries.
- `storyos-server` ran on a new origin (port 8746, `STORYOS_WORKER=0`), so the browser had no storage from the first database. One `storyos-worker --once` ran each request.
- The author created Project `01a1410c-6f12-7b31-a122-6d425ef7298f` ("S4 选区引用旅程"), Volume 第一卷, and Chapter 渡口 `01a1410c-f2af-7072-8081-e992d71206e9` in the editor, and typed two paragraphs by hand. The first admission created the Grant `01a1410e-8ca3-7420-95d7-f1f7e7e25c4e` (`project_writing_assistance_v2`) and one use binding through `create_agent_run`. Nothing was written into the database directly.

### Step 2 with a selection reference, and the part-only request (R15)

- The author selected the first sentence of paragraph 2 ("林远在人群里找了很久，没有找到写信的人。") with the keyboard, clicked "添加到对话", and the composer showed the label 「第 1 章 渡口（第 2 段）」. After the send, the label left the composer, and the history showed it as a sent reference.
- Operation Requirement: `selection_references` has one `chapter_text` reference, `base_authoritative_revision_id` = the Chapter head `01a1410d-315a-7ef3-bea8-77942c68f468`, start = paragraph 2 block offset 0, end = the same block offset 20, `selected_text` = the selected sentence, covered block = paragraph 2 only. `proposal_target_block_ids` = both blocks (Current Chapter and the reference, ADR 0062).
- Context Inspect (public `getAgentRun`): `context.selection_references` shows the same reference with `coordinate_profile` `storyos.editor.utf16-code-unit.v1`, the covered block, and `availability` `current`.
- Wire: the request text has an "Author references" part with the range and the exact text. No `previous_response_id` (first request of the Conversation). No credential.
- Result: `prose_change` with one change, for paragraph 2. Paragraph 1 has no operation. This is the live evidence of #1161: a reply that changes 1 of 2 declared target blocks opened Proposal `01a1410e-d449-7e70-99b9-f28d84972647`, and the panel showed "这次修改涉及 1 个章节、1 处文字".
- Author check (ADR 0062 has no mechanical check): the candidate replaces the selected sentence with a longer search scene, and the text outside the reference ("他把信放进衣袋，坐上了第一班渡船。") is unchanged. The author accepted it (`acceptProposal` → `authoritative_applied`).
- Step 2: pass on `bda9138a`.

### Step 4 with Chapter and candidate references (R16, R17)

- Copy and paste: real Cmd+C in the editor and Cmd+V in the composer of the built-in browser added nothing, because the automated keys did not reach the system clipboard. As agreed with the coordinator, the author used "添加到对话". The paste path evidence is the browser-source test `selection-reference-paste`.
- R16: the author selected both paragraphs and added them (label 「第 1 章 渡口（第 1–2 段）」), then sent "把这两段各自改写得更安静一些。". The reference is `chapter_text` from paragraph 1 offset 0 to paragraph 2 offset 93, on the new head. The continuation was `incremental` with `previous_response_id` = R15. The reply changed both blocks, and Proposal `01a14110-e0f6-7731-884a-521cd368f368` showed two inline locations.
- R17: the author selected the first sentence of candidate 1 in the editor and added it (label 「第 1 章 渡口（第 1 段的修改建议）」), then sent "把这条修改建议缩短一半。". The reference is `proposal_candidate_text` with the Proposal, Operation, and revision of R16, `start_offset` 0, `end_offset` 14, and the exact selected and candidate text. The reply revised that candidate: location outcome `revised`, new Proposal revision `01a14112-15da-7232-aca7-8a8781e432f1`.
- Author actions: the author edited the revised candidate 1 by hand (`applyAuthorEdit` → `proposal_revised`), accepted it (`acceptProposal`, Acceptance Receipt `authoritative_applied`), and rejected location 2 (`rejectProposalOperations` → `proposal_operations_resolved`, Rejection Receipt `author_declined`). Author Action entries 1–9 are all `forward` and match the Receipts.
- Step 4: pass on `bda9138a`.

## Observations of #396

- In the rerun, the "添加到对话" control did not show for a selection in the Chapter that the author had just created and typed. After a page reload it showed. This was seen once and its cause was not examined. Reported to the coordinator.
- A malformed JSON reply becomes an Advisory. This is contract behavior; the UX follow-up is #1145. Not blocking.
- Outline selection was read-only by Stage 2 design; the UX follow-up #1146 was delivered on `main` `020e5575`. Since then, a click on a Chapter makes it current, and the "设为当前章节" menu item is removed. This journey ran before `020e5575` and used "设为当前章节". Steps 4 and 7 do not run again: #1146 changes only how the author changes the current Chapter. The Proposal flow and the editor flow with no model destination did not change.

## Gaps

| Issue | Finding | Effect on S4-JRN-001 |
| --- | --- | --- |
| #1145 | Malformed reply UX | None |
| #1146 | Outline selection is read-only | None; delivered on `020e5575` |
| #1147 | A stale candidate target of a rejected Proposal makes the next send fail with "无法提交请求，请稍后再试。". Work-around: click ordinary prose to clear the focus, then send. | Closed as superseded by #1165 |
| #1148 | The successor result has no decision | Fixed by PR #1150; the second step 5 rerun on `4c29bb6e` passes |
| #1161 | A reply that changes only some declared targets gives no Proposal | Fixed by PR #1162 (`e5a58b17`); R15 on `bda9138a` is the live evidence |
| #1165 | The author decided to replace the caret-based candidate target with a selection added to the Conversation (S4-08) | Delivered on `bda9138a`; the step 2 and step 4 rerun passes |
| #1151 | Writer takeover leaves the base snapshot id out of `project_snapshots`, so the editor becomes read-only | Fixed by PR #1154 (ADR 0060); the takeover rerun on `6d511d93` passes |

## Superseded contract text

The S4 user stories that mention a Keychain credential store follow ADR 0050: the Keychain path was removed in #1099, and the credential reference is `settings:STORYOS_AGENT_PLAN_API_KEY`.

## Claim boundary (S4-REQ-007, S4-EVD-006)

StoryOS claims only its own disclosure and recovery evidence. These stay unclaimed:

- Provider attention to any input item, and how the model used the stored previous response.
- Provider retention and training, other than the documented 72-hour stored-response window that this run used as a fault source.
- Hidden SDK or gateway behavior, and the implicit cache of the destination (reported 0 cached tokens proves no hit).
- Model quality and literary quality of each reply.
