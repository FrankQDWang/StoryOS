# Stage 6 standard Skill journey S6-JRN-001 (2026-11-15)

This record is the live evidence of [S6-10: Verify the Standard Skill Journey](https://github.com/FrankQDWang/StoryOS/issues/415) for [Deliver Stage 6](https://github.com/FrankQDWang/StoryOS/issues/364). It records what StoryOS did on the real Agent Plan route with the two fixture Skills of S6-01a. It has the setup, the live request log, each journey step, the deviations, the exact-dist journey, the requirement and evidence crosswalk, the findings, and the claim boundary.

Claim ceiling: implementation evidence completeness only (HND-003). This record does not emit HND-004, EV-SR, or PASS-STAGE. The parent #364 evaluates them.

## Setup

| Item | Value |
| --- | --- |
| Product code | `stage6-skills` at `2b50cdac95fbe35630c6bf33c1f743ba5d4ea86b`, the tip that holds each child of #364. The branch of this ticket changes no product file. |
| Release package | `python3 scripts/package-release.py` on that clean commit, tree `b3ee30d624cd9858c618f3d3f9df3a19a5b28227`, manifest `sha256:1474d98340a7cc409b3a3350864cab25f1a0745da8ecb4ac4049b976e36341af`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_SKILLS_DIR` on a copy of `fixtures/skill-packages`, on `127.0.0.1:8749`; one `storyos-worker --once` for each Run with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with `env -i` and only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved the key from the settings env file of the main checkout through its typed settings. This run never read, copied, or printed that file or a key. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture, removed after the run. The only direct writes were the guard schema `live_guard` and its cap. The Project, the Volume, the Chapter, the text, each Run, each Proposal decision, and each control went through the panel. |
| Request guard | The `live_guard` triggers of `docs/research/agent-plan-live-compaction-2026-11-14/evidence/request-guard.sql` with a cap of 25 model requests and 0 Exa exchanges. The insert of a Model Attempt counts one request at its claim. Above the cap the trigger refuses the claim transaction, so nothing goes out. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa21`, mapping `storyos.volcengine-agent-plan.responses.create.v7`, grant scope `project_writing_assistance_v4`. Each request declared `web_search`, `web_fetch`, `propose_edit`, `submit_research`, `read_tool_result`, `load_skill`, `read_skill_resource`, and `finish`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`, with `localStorage`, `sessionStorage`, and IndexedDB of the origin clear before step 1. Text went in with the keyboard events of the browser tool. New Project "S6 技能旅程" `01a1de36-9710-72e1-a55f-03e76fbfca89`, Volume 第一卷, Chapter 渡口 with two paragraphs typed in the editor; the second paragraph had dialogue with “” and the tags 喊道 and 冷笑道. |
| Raw evidence | The wire payload of each request (218 KB), the ToolCalls, the `getAgentRun` reads before and after the operator change, the Server and Worker logs, the data dump, and the Project Export Archive (1.2 MB) are outside the repository, in `s6-10-live-evidence/` of the shared notes folder of the coordinator session. |

## Plan

The plan is [#415 comment 6891366444](https://github.com/FrankQDWang/StoryOS/issues/415#issuecomment-6891366444), written before the first request: 19 requests, about 107,000 reported input tokens, guard cap 25. The limits of the ticket are 30 requests and 400,000 reported input tokens. The log is in comments [6891411082](https://github.com/FrankQDWang/StoryOS/issues/415#issuecomment-6891411082), [6891440599](https://github.com/FrankQDWang/StoryOS/issues/415#issuecomment-6891440599), and [6891513809](https://github.com/FrankQDWang/StoryOS/issues/415#issuecomment-6891513809).

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported.

| # | Time | Run, RunStep | Request ID | Result | Usage |
| --- | --- | --- | --- | --- | --- |
| 1 | 07:59:30 | R-A, 0 | `resp_021794729571077f3d3e58bfdffea9769710a7767b276c3bff390` | `propose_edit` with malformed JSON, refused `malformed_arguments` | 3,057 / 1,614 |
| 2 | 08:00:16 | R-A, 1 | `resp_021794729616311dc6263e9e52ba05a43f20bd75063a4f82ff6fc` | `propose_edit` with malformed JSON, refused `malformed_arguments` | 4,545 / 257 |
| 3 | 08:00:23 | R-A, 2 | `resp_021794729623681e41c82dd1debd95d24831350e3c3a877e9efc0` | `propose_edit`, Proposal opened | 4,647 / 238 |
| 4 | 08:00:29 | R-A, 3 | `resp_021794729629994f3d3e58bfdffea9769710a7767b276c3c3fd92` | `finish` | 4,200 / 889 |
| 5 | 08:00:53 | R-A, 4 | `resp_0217947296533736e0aa4b8e2b3fc35d377306409bf441986ad90` | final answer | 5,140 / 332 |
| 6 | 08:02:02 | R-B, 0 | `resp_021794729722954af71e85a49ed90f7564e7e8026279fe7c39d50` | `load_skill` `internal-comms` | 5,004 / 139 |
| 7 | 08:02:07 | R-B, 1 | `resp_021794729727218cfacb7e54fe5570d0ae3d0783d99a7703cc084` | `read_skill_resource` `examples/3p-updates.md` | 5,509 / 93 |
| 8 | 08:02:11 | R-B, 2 | `resp_021794729731280ef74dd90a5750f77dc784a69dff0f3ad53797a` | `finish` | 6,380 / 138 |
| 9 | 08:02:16 | R-B, 3 | `resp_0217947297370966cb63cadb26a6eb1a5dd8bb71cce3d7f576942` | final answer | 6,567 / 173 |
| 10 | 08:02:51 | R-C, 0 | `resp_021794729771644c37b19e6f2deafccdd0d14a1cba0df7b3ac023` | `load_skill` `dialogue-style-guide` | 7,236 / 805 |
| 11 | 08:03:13 | R-C, 1 | `resp_02179472979337709b5057e6c5823bee4bfc995e78b34aef0cffb` | `finish` | 8,416 / 48 |
| 12 | 08:03:15 | R-C, 2 | `resp_0217947297959219714750b4c92df19b7d86c8470c87ebf568ef0` | final answer | 8,511 / 278 |
| 13 | 08:04:41 | first send of step 6, 0 | `resp_0217947298815313dcd3c0a2d24a55cb96c7787eab353825a3d8c` | `finish` with no load (finding F2) | 8,406 / 1,249 |
| 14 | 08:06:01 | R-D, 0 | `resp_02179472996205791eb505e0b3b4db7bd42120475941dff876e7f` | `load_skill` `dialogue-style-guide` | 2,780 / 153 |
| 15 | 08:06:48 | R-D, 2 | `resp_02179473000848161e61b6be4ea80d80d16a59ce3c2929cc87e7f` | `finish`, after the operator change | 3,685 / 1,238 |
| 16 | 08:07:20 | R-D, 3 | `resp_0217947300410452fe30231c3416749cd01035ba2ea7b618a278e` | final answer | 4,973 / 499 |
| 17 | 08:08:26 | R-F, 0 | `resp_0217947301063054e0d847e114bd906533eadfa024bfe6dcdb0bc` | `load_skill` `internal-comms`, refused `skill_not_in_catalog` | 4,572 / 1,753 |
| 18 | 08:09:14 | R-F, 1 | `resp_021794730155045276b7ea097d56db2840fe05b4d1a72919e0bf8` | `propose_edit`, Proposal opened | 6,418 / 629 |
| 19 | 08:09:28 | R-F, 2 | `resp_0217947301690407f7defdca68741f744da3a9cb51804c44fe0bc` | `finish` | 5,449 / 263 |
| 20 | 08:09:37 | R-F, 3 | `resp_0217947301780123a5ef8b5d1eba84c17b54858e5dd40fdae4409` | final answer | 5,759 / 287 |

Total: **20 model requests (plan 19, cap 25), 111,254 reported input tokens (plan about 107,000), 11,075 output tokens, 0 Exa exchanges, no Retrieve, no recovery.** Each Model Attempt is `settled`.

**Anomaly: one request more than the plan.** The delivery document ("Live model requests") makes a request count above the plan an anomaly.

| Step | Planned | Sent | Requests |
| --- | --- | --- | --- |
| 1. Select and run (R-A) | 3 | 5 | 1 to 5; two more because of finding F1 |
| 2 and 3. Load and read (R-B) | 4 | 4 | 6 to 9 |
| 4. Two Skills in one turn (R-C) | 5 | 3 | 10 to 12; no read and no extra Tool call |
| 6. A change by the operator (first send, R-D) | 4 | 4 | 13 to 16; request 13 is the cancelled first send (finding F2) |
| 7. Continue without the Skill (R-F) | 3 | 4 | 17 to 20 |
| **Total** | **19** | **20** | |

Steps 0a, 0b, 5, and 6a send no request, as planned.

The one more request is request 17, `resp_0217947301063054e0d847e114bd906533eadfa024bfe6dcdb0bc`: the refused `load_skill` call of the removed `internal-comms` in step 7 (finding F5). The totals stayed below the guard cap of 25 and the ticket limits of 30 requests and 400,000 reported input tokens. The executor reported the anomaly to the coordinator after Spec review round 1.

Disposition of the coordinator, 2026-11-15, in the body of #415:

> the journey sent 20 requests against a plan of 19 (111,254 reported input tokens against about 107,000). The one more request is the refused `load_skill` call of the removed Skill in the step "Continue without the Skill". The executor reported it as the anomaly that the delivery document names. It is accepted with no second run: the totals are inside the limits of this criterion (30 requests, 400,000 tokens) and inside the guard of the plan (25 requests), each request is in the log, and the cause is a choice of the model that the record gives as a finding. The record states the planned count, the sent count, and this disposition.

Run identifiers: R-A `01a1de37-ac70-7e51-bd98-f1f1ceaf9ad6`, R-B `01a1de3a-3a6d-7e10-8575-1c05719da4d9`, R-C `01a1de3a-f85c-7ef2-aaa6-a1a09bbaeab1`, first send of step 6 `01a1de3c-a5ce-7a51-84f3-65ab88f2bfa9` (cancelled), R-D `01a1de3d-e03a-77d0-a93a-96a438805f64`, R-F `01a1de40-13d1-72c2-9f03-907c824e1d63`. R-A to the first send of step 6 are in Conversation `01a1de37-ac70-7e51-bd98-f1efbbf95539`; R-D and R-F are in the new Conversation `01a1de3d-e03a-77d0-a93a-969e17e42ff7`.

Key check: the two Server logs, the Worker logs, a data dump of the `storyos` and `live_guard` schemas (1,363 lines), and the files of the Project Export Archive have no `Authorization` header, no bearer token, and no `x-api-key`. The matches of "authorization" in the dump are the column `qualification_authorization`.

## Journey steps

| Step | S6-JRN-001 item | What the author did | What the panel showed and what `getAgentRun` gives |
| --- | --- | --- | --- |
| 0a | Refusal of a package with no `SKILL.md` (no request) | Operator: a copy of the directory with one more directory `no-skill-md` that holds only `README.md`. | The Server exited with code 1: `Error: STORYOS_SKILLS_DIR: the Skill package "no-skill-md" is refused: skill_md_missing`. |
| 0b | Refusal of a `SKILL.md` above 32,000 tokens (no request) | Operator: a copy with one more package `long-guide` whose `SKILL.md` has 200,066 bytes (40,000 words). | The Server exited with code 1: `Error: STORYOS_SKILLS_DIR: the Skill package "long-guide" is refused: skill_md_too_long`. |
| 1 | Select and run | Typed `$`, clicked `dialogue-style-guide` in the picker, then typed "请按规范改写这一章第二段的对白，直接修改正文。" and clicked 发送. Accepted the Proposal in the editor. | The composer showed the chip and the typed text. The stored author message is exactly the typed text, one time. The Operation Requirement of RunStep 0 is `[{"provenance":"author_selected","skill_name":"dialogue-style-guide","package_digest":"sha256:e03399e0…8178b"}]`. The `getAgentRun` read of Model Attempt `01a1de37-f24d-…` (RunStep 0) selects `skill_instruction` with source version `sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b` and the exact bytes of the fixture `SKILL.md`, and `skill_catalog` `sha256:e5028dfa…`. Run `completed`, decision `prose_change`, results `proposal` and `answer`. The candidate uses 「」 and only 说 (rules 1 and 2). Panel: 这次修改涉及 1 个章节、1 处文字 and the line 使用了你选择的技能「dialogue-style-guide」. |
| 2 and 3 | The Agent loads a Skill and reads a resource | No chip: "请帮我给编辑写一份本周的写作进度周报，按进展、计划、问题三部分来写：…直接在回答里给我周报全文，不要修改正文。" | RunStep 0: `load_skill` `{"skill":"internal-comms","reason":"作者需要写给编辑的写作进度周报，属于内部状态汇报，适用内部沟通技能的格式规范。"}`, `settled`, no Destination Attempt. RunStep 1: `read_skill_resource` `{"skill":"internal-comms","path":"examples/3p-updates.md"}`, a file that the `SKILL.md` names, `settled`, no Destination Attempt. The read of RunStep 2 selects `skill_instruction` (source version: the load ToolCall) with the exact `internal-comms/SKILL.md` bytes and `skill_resource` with the exact bytes of `examples/3p-updates.md`. Run `completed`, decision `advisory`. Panel: 使用了技能「internal-comms」：作者需要写给编辑的写作进度周报，属于内部状态汇报，适用内部沟通技能的格式规范。 |
| 4 | Two Skills in one turn | Typed `$`, clicked `internal-comms`, then typed "给编辑写一份简短的进度更新，并在更新里附上一段林舟和林雪回家路上的示范对白（四到六句）。直接在回答里给我全文，不要修改正文。" | The stored author message is exactly the typed text. RunStep 0 selects `skill_instruction` `sha256:3ccc24f2…` (`author_selected`). The model loaded `dialogue-style-guide` with the reason "更新中需要新写一段林舟、林雪的示范对白，应按对白写法规范来写。". RunStep 1 selects two separate `skill_instruction` items: `sha256:3ccc24f2…` (1,511 bytes) and the load output (source version: ToolCall `01a1de3b-5656-…`); there is no merged item. `skill_uses`: `author_selected` `internal-comms`, then `agent_selected` `dialogue-style-guide` with the reason. The answer has five lines of dialogue with 「」 and only 说 and 问. |
| 5 | The usage record | Read the panel, then reloaded the page. | Before and after the reload the panel has the same three lists: (1) 使用了你选择的技能「dialogue-style-guide」; (2) 使用了技能「internal-comms」：<reason>; (3) 使用了你选择的技能「internal-comms」 and 使用了技能「dialogue-style-guide」：<reason>. No line has a digest or an identifier. |
| 6 | A change by the operator with a Run in progress | First send in the same Conversation did not load a Skill (finding F2); the author cancelled it (取消任务) and sent "按小说对白的写法规范，检查这一章里的人物对白是否自然，告诉我哪里需要改，先不要修改正文。" after 新对话. The guard cap was set to the count + 1, so after the `load_skill` of RunStep 0 the guard refused the claim of the continuation, and the Worker stopped. The author pressed 暂停 (任务已暂停，候选文字保持原样。). The operator added rule 7 to `dialogue-style-guide/SKILL.md`, removed `internal-comms`, and started the Server again (08:06:31). The author resumed with the guidance "请继续。" (要求已收到，将用于当前任务。). | Before the change R-D pinned `dialogue-style-guide` `sha256:e03399e0…` and `internal-comms` `sha256:3ccc24f2…`. After the restart `listSkills` gives only `dialogue-style-guide` `sha256:6c0f44b4a06e3cf480642fd5f43e38e8d9d729180f12b2388fec83fc274d0ca6`. The continuation (request 15, after the restart) has `previous_response_id` of request 14, the section `Skill catalog` with both pinned Skills (source version `sha256:e5028dfa…`, the same as before), and a load output that is byte for byte the earlier `SKILL.md` (no rule 7). R-D `completed`; `skill_uses` `agent_selected` `dialogue-style-guide`. Each `getAgentRun` read of R-A, R-B, and R-C, and of each of their 12 Model Attempts, is equal to the read before the change (15 JSON files compared, the correlation identity left out). |
| 6a | A stale `$` chip after the restart (no request) | With no reload, typed `$`; the picker still listed both Skills from its read before the restart. Clicked `internal-comms`, typed "在第一段末尾补一句林雪寄给出版社编辑的周报摘要，按进展、计划、问题来写，直接修改正文。", and clicked 发送. | `createAgentRun` got HTTP 422 `skill_selection_unavailable`. The status line is 所选技能现在不可用，消息没有发送。请重新选择技能，或直接发送。, the message text stayed in the composer, and the chip went away. |
| 7 | Continue without the Skill | Clicked 发送 again with the same text and no chip. Rejected the Proposal in the editor. Typed the paragraph "两人沿着江堤往回走，谁也没有先开口。" at the end of the Chapter; it saved, and a reload showed it. | R-F pinned only `dialogue-style-guide` `sha256:6c0f44b4…`. Each of its four requests selects only `skill_catalog` `sha256:0fbd9e68…`, whose text has no `internal-comms`. The model called `load_skill` `internal-comms`; the Host refused it with `skill_not_in_catalog`, and the Run continued: `propose_edit`, `finish`, answer. R-F `completed`, decision `prose_change`, no `skill_uses`; the panel shows no Skill line for this turn. |
| — | Project Export Archive | Public `exportProjectArchive` and one `storyos-worker --once` with `host_fake` (no model request), because the panel has no archive export control. | Export `01a1de43-44a7-7c60-8c8e-20f63da33614`, 73 files. `canonical/skill_package_snapshots.json` 3 rows: `internal-comms` `sha256:3ccc24f2…`, `dialogue-style-guide` `sha256:e03399e0…`, and `dialogue-style-guide` `sha256:6c0f44b4…` (the public package after its removal from the directory). `skill_package_snapshot_files.json` 10, `agent_run_skill_catalog_entries.json` 11, `tool_calls.json` 15 (5 of them `load_skill` or `read_skill_resource`). Each count is equal to the database. |

The Project has these Receipts: `createProject` 1, `createVolume` 1, `createChapter` 1, `applyAuthorEdit` 7, `undoLatestAuthorAction` 2, `createAgentRun` 6, `acceptProposal` 1, `rejectProposalOperations` 1, `pauseAgentRun` 1, `steerAgentRun` 1, `cancelAgentRun` 1, `exportProjectArchive` 1. The refused stale chip wrote no Receipt.

### Finding 2 of S6-03c

In the exact-dist journey and in steps 1 and 4 of the live journey, the author selected a Skill with the `$` picker first and typed the message after that, with keyboard events. In each case the composer showed the typed text, and the author message of `createAgentRun` was exactly that text, one time (exact-dist: the request body; live: the stored author message of R-A and R-C). The composer has no defect here: finding 2 of the live record of S6-03c was an effect of the automated typing of that run, not of the composer.

### Deviations

- Step 6: the guard cap (count + 1) stopped the Worker before the continuation, in place of a stop of the Worker process. The refused claim sent nothing and wrote nothing. The author then paused and resumed the Run in the panel. Resume needs a guidance message, because the panel has no other resume control; "请继续。" became the steering input of RunStep 1.
- Step 6: the first send in the same Conversation did not load a Skill (finding F2). The author cancelled that Run and sent the step one more time, inside the planned count.
- Step 7: the first try to put the caret at the end of the Chapter with Cmd+End put the text in the middle of paragraph 2. The author pressed Cmd+Z two times (two `undoLatestAuthorAction` Receipts), and the Chapter was the same as before. Then the author clicked at the end of the last line and typed the paragraph.
- The Project Export used the public command, as the Stage 5 journey did.

## Exact-dist journey

`apps/web/test/browser-exact-dist/skill-journey.integration.test.ts` runs the scenario `skill_journey` of the production host command (`apps/web/test/support/production-skill-journey-command.ts`). It starts its own packaged Server with `STORYOS_SKILLS_DIR` on `fixtures/skill-packages` and the fake model destination, and the packaged Worker with real PostgreSQL. It first settles each Run that an earlier journey left in the queue. It checks:

1. The author types `$`, clicks `internal-comms`, and then types the message with keyboard events. The composer shows the chip and the typed text, and the `createAgentRun` request has exactly that text and `skill_selections` `[internal-comms]`.
2. In that one Run the fake model loads `dialogue-style-guide` and reads `references/examples.md`. Both ToolCalls are `settled` with no Destination Attempt. RunStep 0 selects the exact `SKILL.md` of `internal-comms` with its package digest. RunStep 2 selects both `SKILL.md` texts and the exact file. After a reload the panel shows 使用了你选择的技能「internal-comms」 and 使用了技能「dialogue-style-guide」：<reason>.
3. A `load_skill` of `no-such-skill` is refused with `skill_not_in_catalog`, the Run completes with no `skill_uses`, and only the history turn of the first Run has Skill lines.
4. The picker shows an earlier digest of `dialogue-style-guide` (the list read of the panel is changed, as after a change by the operator). A chip with it gives HTTP 422, the one sentence, the same text in the composer, and no chip. The same text sent again completes a Run.
5. The Project Export Archive and the Project Receipts. The public `exportProjectArchive` command and the Worker export the archive. Its tables hold 2 SkillPackage Snapshots (the two fixture digests), 8 Snapshot files, 6 pinned catalog entries (two for each of the three Runs), and the 3 Skill ToolCalls, the same counts as the database. The Project Receipts are exactly the actions of the journey.

The limits (8 loads, 16 reads, 32 Skills) and the restart with a changed directory stay with the tests of their owners (ADR 0075).

The physical recovery drill (`scripts/verify-recovery-hold.sh`, in `make check-full`) now runs this journey on the primary database before the backup. The fact function `stage6_skill_facts` records the count and a digest of the SkillPackage Snapshots, their files, the pinned catalogs, and the ToolCalls of `load_skill` and `read_skill_resource`. It refuses an empty family and compares them after the restore. The fact function `stage6_skill_export_facts` records the export, the immutable root, the size, and a digest of the four Skill tables of the Project Export Archive of this journey (`skill_package_snapshots`, `skill_package_snapshot_files`, `agent_run_skill_catalog_entries`, `tool_calls`). It refuses a missing table and compares them after the restore.

The Stage 2 AI-disabled journeys (`s2-jrn-001` and `s2-workspace`) run in `make check-full` against a Server with no model destination. That Server now has `STORYOS_SKILLS_DIR` on the fixtures (`start_ai_disabled_exact_dist_server`), so they show that such a Server starts and serves the manual editor in each state.

## Requirement and evidence crosswalk

Each row names the delivered child evidence on `stage6-skills` `2b50cdac`. Each named test is in the `make check` set or the `make check-full` set of the head of this ticket, which changes no product file, so the evidence is current. Each PASS holds only with the `make check-full` Check Evidence Line of the pull request head of this ticket.

| Row | Delivered evidence | Journey evidence (this record) | Result |
| --- | --- | --- | --- |
| `S6-REQ-001` | S6-01a #406 (PR #1534): the directory and the package rules (`skill_package_tests`, `skill_directory_tests`, `skills_dir_startup::the_fixture_skill_directory_passes_and_the_start_goes_on_to_the_web_assets`). S6-01b #1527 (PR #1536): the Snapshot and the pinned catalog (`each_run_pins_the_set_of_its_admission_and_only_new_digests_get_a_snapshot`, `skill-package-snapshots-http`). S6-02b #407 (PR #1537): exact selection (`skill-selection-http`, `skill_selections_tests`). S6-02c #1529 (PR #1540): `listSkills` and the picker (`list-skills-http`, `assistant-skill-picker`). S6-03b #408 (PR #1541): selection by the Agent with a reason (`a_load_settles_with_the_exact_skill_md_and_the_next_request_sends_it_as_the_call_output`). The second installation scope is waived (ADR 0089). | Steps 1 to 4; exact-dist `skill-journey` | PASS |
| `S6-REQ-002` | S6-02a #1528 (PR #1535): the source classes, the grant scope v4, the Host paragraph (`skill_wire_tests`). S6-03a #1530 (PR #1539): the catalog item (`each_run_step_selects_the_catalog_and_only_a_selected_run_selects_the_exact_skill_md`). S6-03b #408: the load output below the Host and Project instructions. S6-03c #1531 (PR #1543): bounds and joint use (`nine_loads_of_one_response_settle_eight_and_refuse_the_ninth_at_the_limit`, `two_loads_are_two_outputs_in_call_order_and_each_full_input_sends_both_again`, `a_compaction_after_the_loads_keeps_each_load_output_exact_and_names_no_load_stub`), live record `agent-plan-live-skill-load-2026-11-15.md`. S6-03d #1532 (PR #1544): progressive resources (`a_read_of_the_resource_of_each_fixture_settles_with_its_exact_text`, `each_refused_read_gives_its_reason_and_the_run_continues`, `seventeen_reads_of_one_file_settle_sixteen_and_refuse_the_last_at_the_limit`). | Steps 2, 3, 4, 7 | PASS |
| `S6-REQ-003` | Withdrawn for Stage 6 by ADR 0089 (waiver of the parent). No evidence. | — | Waived |
| `S6-REQ-004` | S6-03f #1533 (PR #1542): `skill_uses` and the panel lines (`the_run_read_names_each_selected_skill_then_each_settled_load_with_its_reason`, `skill-uses-http`, `assistant-run-results`). S6-01b #1527: a change of the set applies to later Runs only (`each_run_pins_the_set_of_its_admission_and_only_new_digests_get_a_snapshot`). Each Skill that an author provides, the inspection of composed instructions, conflicts, and outcome obligations, and the Skill Revocation are waived (ADR 0089). | Steps 5, 6, 6a, 7 | PASS |
| `S6-EVD-001` | The unchanged public fixture `internal-comms` (S6-01a, `fixtures/skill-packages/PROVENANCE.md`); exact selection and Snapshot (S6-01b, S6-02b); progressive loading (S6-03b, S6-03d); joint use (S6-03c); instruction authority (S6-02a `a_skill_item_adds_the_skill_paragraph_and_a_catalog_adds_the_load_and_read_paragraphs`). | Steps 1 to 4; exact-dist `skill-journey` | PASS |
| `S6-EVD-002` | A change of the Skills with a Run in progress (S6-01b tests above; S6-03a: each RunStep reads the pinned rows); the refusal of an invalid or oversized package at the start (`skills_dir_startup::an_invalid_skill_package_stops_the_start_with_the_package_and_the_rule`, `a_catalog_text_above_32000_tokens_stops_the_start_with_the_rule`); active-run binding (S6-02b, S6-01b); editor regression (the Stage 2 AI-disabled journeys with a Skill directory, the physical drill with the Stage 6 record families, both in `make check-full` of this head). The Tool-role, script, outcome-obligation, and revocation evidence is waived (ADR 0089). | Steps 0a, 0b, 6, 6a, 7 | PASS |
| `S6-JRN-001` | This record and exact-dist `skill-journey`. | Steps 0a to 7 | see "Journey steps" |

## Findings

No finding blocks S6-JRN-001. Each item is an observation for the parent #364 to triage. The Host got no keyword router and no change of a Host sentence because of them.

| ID | Finding | Effect on S6-JRN-001 | Owner |
| --- | --- | --- | --- |
| F1 | In R-A (requests 1 and 2) the model sent the `propose_edit` arguments as malformed JSON two times (a stray `": "` after a key), and the third call was valid. Each refusal gave the reason, and the Run continued. | Two requests more than the plan of step 1. | Observation (model quality); no Host change. |
| F2 | In the first send of step 6 (request 13), in a Conversation whose earlier turns had loaded `dialogue-style-guide`, the model did not call `load_skill` and answered from the earlier turns. In a new Conversation with a clearer message it loaded the Skill. | One more send, inside the planned count. ADR 0089 keeps no Conversation-level Skill list, so this is the expected effect of the history. | Observation for #364. |
| F3 | In step 4 (requests 10 to 12) the selected `internal-comms` was in the first RunStep, but the model did not read a file of `examples/`; it had read `examples/3p-updates.md` in the earlier turn of the same Conversation. | None: step 3 has the read. | Observation. |
| F4 | In R-A (request 5) the final answer said that the only editable target block was the first paragraph. The second block was then reserved by the open Proposal of the same Run. | None: the Proposal opened and the author accepted it. | Observation. |
| F5 | In step 7 (request 17) the model tried to load `internal-comms` because the earlier turns of the Conversation named it. The Host refused it with `skill_not_in_catalog`, and the Run completed. | None: this is the designed refusal of ADR 0089 section 7. | Observation. |
| F6 | The panel has no Project Export Archive control and no resume control other than a guidance message. | None: the export used the public command, as in Stage 5. | Observation (as in the Stage 5 record). |

## Claim boundary (HND-003)

StoryOS claims only its own Skill provision, selection, loading, Snapshot, usage record, and recovery evidence. These stay unclaimed:

- What the provider did with each request after it left StoryOS, and how the model used each Skill text.
- Provider retention and training.
- Hidden SDK or gateway behavior that StoryOS cannot observe.
- The quality of the model choice and the literary quality of each reply.

This record states HND-003 (implementation evidence completeness) only. HND-004, EV-SR, and PASS-STAGE belong to the parent #364.
