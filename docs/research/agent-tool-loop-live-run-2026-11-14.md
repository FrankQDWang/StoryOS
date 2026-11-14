# Agent tool loop live run (2026-11-14)

This record is the live evidence of [Record a Live Run of the Tool Loop and Close the Specification](https://github.com/FrankQDWang/StoryOS/issues/1464) for the specification [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451) and [ADR 0091](../adr/0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md). It records five Runs in one Conversation on the real Agent Plan route and the real Exa MCP server. Run 2 is the answer Run, Run 4 is the edit Run, and Run 5 is the research Run of the acceptance criteria.

## Setup

| Item | Value |
| --- | --- |
| Product code | `agent-tool-loop` at `ee4c890931c44327b2303b7692321996d46f3356` (tree `190501ed`). This branch changes no product file. |
| Release package | `python3 scripts/package-release.py` on the clean tip above, manifest `sha256:8fb2e3f691e1ba0bcecb293b2b529f2669e44be31ddbe6a57d785aea4e02c5c1`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8743`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package. |
| Settings | ADR 0050: the Worker resolved both keys from the env file of the main checkout. This run never read, copied, or printed that file or a key. The Server log (274 lines), the Worker log (28 lines), and a data dump of the `storyos` schema have no `Authorization` header, no bearer token, and no `x-api-key`. The one match of "authorization" in the dump is the column `qualification_authorization`. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The only direct writes were the guard schema `live_guard`. The Chapter text went through the editor. |
| Request guard | Two triggers of the guard schema: the insert of a Model Attempt counts one model request, and the first `destination_attempt_id` of a ToolCall counts one Exa exchange. Above the cap the trigger refuses the claim transaction, so nothing goes out. |
| Model | `doubao-seed-2-1-pro-260915`, Model Registration `018f0000-0000-7001-8000-00000000fa19`, mapping `storyos.volcengine-agent-plan.responses.create.v5`. Tool Registration revision `018f0000-0000-7001-8000-0000000e0001`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`. The site storage was clear before the first step. Project `01a1d770-f7b7-7d02-8f91-b9723b890ce9` ("工具循环实测"), Chapter 灯塔 `01a1d771-d7ed-7580-a185-a990b63432bf` with three paragraphs, Conversation `01a1d772-80d2-7640-86a8-ee40b2890f7b`. |

## Plan

Written before the first request.

- Purpose: show the finish rule, the Host Tools, the result list of a Run, and the continuation of the Provider conversation between Runs on the real model.
- Run 1, answer: `finish` (RunStep 1), then the closing request. Plan: 2 model requests.
- Run 2, edit: `propose_edit`, `finish`, then the closing request. Plan: 3 model requests.
- Run 3, research: `web_search`, `web_fetch`, `submit_research`, `finish`, then the closing request. Plan: 5 model requests and 2 Exa exchanges.
- Total plan: 10 model requests and 2 Exa exchanges. Automatic requests (a Retrieve, a compaction request, a correction after a refusal) count.
- Hard cap: 20 model requests, 8 Exa exchanges, 28 in all. A database trigger of the disposable database refuses each claim above the cap, so nothing goes out.

Plan change at 00:28 UTC, after Run 1 and before the next request. The question of Run 1 asked about a ship sound signal, and the model used web research for it. Run 1 used 4 model requests and 2 Exa exchanges and gave a Research Artifact and an answer. Thus Run 1 is not the answer Run. New plan for the next Runs, in the same Conversation: Run 2 answer (2 model requests), Run 3 edit (3 model requests), Run 4 research (5 model requests, 2 Exa exchanges). New total plan: 14 model requests and 4 Exa exchanges. The hard cap does not change.

Plan change at 00:33 UTC, after Run 3 and before the next request. Run 3 made no Proposal: its request had no working text and no target blocks (finding F1). Thus the edit Run occurs again with a message that names the Chapter: Run 4 edit (3 model requests), Run 5 research (5 model requests, 2 Exa exchanges). Count after Run 3: 11 model requests and 2 Exa exchanges. New total plan: 19 model requests and 4 Exa exchanges. The hard cap changes to 28 model requests, 8 Exa exchanges, 36 in all, because the old cap of 20 left no space for one correction in Run 5. The trigger changed before the next request.

Final count: **19 model requests and 4 Exa exchanges, 23 of 36**, equal to the last plan. No Retrieve, no compaction request, and no recovery occurred. Each Model Attempt is `settled`.

## Live request log

Each row is one claim that the guard counted. The time is the claim time (UTC). The request ID of a model request is the complete response reference of the Provider. The request ID of an Exa exchange is its ToolCall. Usage is input / output tokens as the Provider reported them. `prev` is the end of the `previous_response_id` of the request. The mapping is the continuation input mapping.

| # | Time | Run, RunStep | Request ID | Mapping, `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 00:26:08 | 1, 0 | `resp_021794615968277af71e85a49ed90f7564e7e8026279fe7b069df` | none, none | required | 2 `web_search` calls in one response | 1753 / 1464 |
| E1 | 00:27:11 | 1, 0 | ToolCall `01a1d773-7879-72d2-8064-53c3d8ca2af4` | — | — | `web_search` settled, `confirmed`, 15,066 bytes | — |
| E2 | 00:27:15 | 1, 0 | ToolCall `01a1d773-787e-7652-978d-64b29b3e1f49` | — | — | `web_search` settled, `confirmed`, 13,556 bytes | — |
| M2 | 00:27:19 | 1, 1 | `resp_021794616039263c280e9bf492d190027beb8b799de38086c9740` | incremental, M1 | required | `submit_research` settled | 11153 / 616 |
| M3 | 00:27:37 | 1, 2 | `resp_021794616057223841fc2298faf73f2fc5d524f9c961e71fac499` | incremental, M2 | required | `finish` counted | 11838 / 40 |
| M4 | 00:27:40 | 1, 3 | `resp_021794616060473f4d7ae5d724d9e41b76430b6d79530bc94a554` | incremental, M3 | none | final answer | 11926 / 207 |
| M5 | 00:29:58 | 2, 0 | `resp_0217946161986366b3b42eb0dccc67d054a95217969ba77d038b1` | incremental, M4 | required | `finish` counted | 10836 / 155 |
| M6 | 00:30:05 | 2, 1 | `resp_02179461620597824e994d08a876ec3f4dc3077f2522d845711c4` | incremental, M5 | none | final answer | 11059 / 48 |
| M7 | 00:30:29 | 3, 0 | `resp_0217946162294374f170dda172df2cc79a65077ac74d4048a5bdb` | incremental, M6 | required | `propose_edit` refused `undeclared_block` | 11001 / 525 |
| M8 | 00:30:48 | 3, 1 | `resp_021794616248432662416cb134b03a75ced108817244360d230c7` | incremental, M7 | required | `propose_edit` refused `undeclared_block` | 11613 / 556 |
| M9 | 00:31:05 | 3, 2 | `resp_0217946162651986d6c4a2c22a8ac0107afc732836cd422850eb7` | incremental, M8 | required | `propose_edit` refused `undeclared_block` | 12236 / 890 |
| M10 | 00:31:33 | 3, 3 | `resp_02179461629325473b8dfe497f31d2aa905b6fe2ab6c6817e7a59` | incremental, M9 | required | `finish` counted | 13194 / 723 |
| M11 | 00:32:01 | 3, 4 | `resp_021794616321463c280e9bf492d190027beb8b799de3808194bbe` | incremental, M10 | none | final answer | 13966 / 237 |
| M12 | 00:34:20 | 4, 0 | `resp_0217946164604242f58812a10f9c6b8f25a472cef1ad3e5225ec6` | incremental, M11 | required | `propose_edit` settled, Proposal opened | 12761 / 307 |
| M13 | 00:34:30 | 4, 1 | `resp_021794616470202f4d7ae5d724d9e41b76430b6d79530bc57771c` | **full, none** | required | `finish` counted | 2054 / 97 |
| M14 | 00:34:36 | 4, 2 | `resp_0217946164771196216202cdc086c00ba26009437f217e9ca4412` | incremental, M13 | none | final answer | 2201 / 75 |
| M15 | 00:35:05 | 5, 0 | `resp_021794616505994c35f1a31989945a684cc428e09e81c794659c3` | incremental, M14 | required | `web_search` | 2439 / 109 |
| E3 | 00:35:10 | 5, 0 | ToolCall `01a1d77a-c7b2-7180-b20b-f76fe1f3c0b0` | — | — | `web_search` settled, `confirmed`, 21,765 bytes | — |
| M16 | 00:35:13 | 5, 1 | `resp_0217946165141128b180e7ab5368500e338034e77cb755d6a93ab` | incremental, M15 | required | `web_fetch` of 2 URLs | 8319 / 134 |
| E4 | 00:35:19 | 5, 1 | ToolCall `01a1d77a-eb51-7e62-acae-b7432dd847dc` | — | — | `web_fetch` settled, `confirmed`, 6,207 bytes | — |
| M17 | 00:35:23 | 5, 2 | `resp_02179461652345861e61b6be4ea80d80d16a59ce3c2929cec60c7` | incremental, M16 | required | `submit_research` settled | 10165 / 1520 |
| M18 | 00:36:04 | 5, 3 | `resp_0217946165645836e0aa4b8e2b3fc35d377306409bf44192c5943` | incremental, M17 | required | `finish` counted | 11752 / 39 |
| M19 | 00:36:07 | 5, 4 | `resp_02179461656748024e994d08a876ec3f4dc3077f2522d844ea682` | incremental, M18 | none | final answer | 11839 / 685 |

Usage total: 182,105 input tokens and 8,427 output tokens in 19 model requests.

## Runs

| Run | Run ID | Author message (short) | Model requests | Usage in / out | Tool calls | End | `results` of the Run read |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `01a1d772-80d2-7640-86a8-ee59375305f4` | What can "一声长，两声短" mean? | 4 | 36,670 / 2,327 | 2 `web_search`, `submit_research`, `finish` | `completed` after a counted `finish` and a closing response with no call | `research_artifact` `01a1d773-dc72-7a93-9a18-884cf5f65ebd` (10 Source Snapshots, 0 gaps), then `answer` |
| 2 | `01a1d776-04d9-7b83-80e0-1fd4c680bf24` | Why does 林三 ring the bell? No web. | 2 | 21,895 / 203 | `finish` | `completed`, as Run 1 | `answer` |
| 3 | `01a1d776-7d2d-7093-af91-051cad48f668` | "请修改第三段：把林三敲钟的动作写得更具体，写出他的手和呼吸，并按雾航规则的意思让他一直敲下去。只改这一段，不要联网。" | 5 | 62,010 / 2,931 | 3 `propose_edit` (refused), `finish` | `completed`, as Run 1 | `answer` that says the Host refused the change three times, with the suggested text |
| 4 | `01a1d77a-037c-7df1-8c5f-878960d05868` | "把本章最后一段林三敲钟的动作写得更具体：写出他的手和呼吸，并按雾航规则的意思让他一直敲下去。只改这一段，不要联网。" | 3 | 17,016 / 479 | `propose_edit`, `finish` | `completed`, as Run 1 | `proposal` `01a1d77a-29b9-7421-aea3-4004b817ba38` (1 location, block `01a1d772-18ba-77ad-a14e-f4da491bb907`, `opened`, `pending`), then `answer` |
| 5 | `01a1d77a-b54a-7c20-95a8-c55fe912efb7` | Weight, interval, and drive of 19th-century fog bells | 5 | 44,514 / 2,487 | `web_search`, `web_fetch`, `submit_research`, `finish` | `completed`, as Run 1 | `research_artifact` `01a1d77b-9a3d-7f92-91be-558bab4ac71b` (10 Source Snapshots, 0 gaps, 4 claims), then `answer` |

- The panel showed the same results in the same order for each Run: the research turn with its sources, claims, citation list, and "用到故事里", then the answer text; for Run 4 the Host sentence "这次修改涉及 1 个章节、1 处文字", the change location, and the answer. The history turns of Runs 1 to 4 showed the same results as the current turn.
- Each Run ended as the finish rule expects. Each request before a counted `finish` had `tool_choice: "required"`, each closing request had `tool_choice: "none"`, and no closing response held a function call. No Run completed with no result. No Run reached the limit of 24 RunSteps.
- Continuation: the first request of each of Runs 2 to 5 sent the closing response of the Run before it as `previous_response_id` (M5, M7, M12, M15). Each function output of a continuation has the `call_id` of its call.
- Refused calls: M7, M8, and M9 each had one `propose_edit` call that the Host refused with the label `undeclared_block`. The model did not correct the call: it sent the same block identity again, and in M9 also the two other blocks with their text unchanged. After the third refusal it called `finish` and told the author in the final answer that no Proposal opened, as ADR 0091 section 5 asks. No call got `changed_head` or `stale_candidate`.
- Compaction: no Run compacted. Thus no live request covered the form after a compaction at RunStep 1 or later (coordinator note 2 on #1464).

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| F1 | An author message that starts with "修改", "改写", "润色", or "精简" (also after "请") goes through `parse_ordinary_passage_request`, the passage parser of the Host-fake profile `storyos.host-fake.passage-resolution.v1`, also on the real route (`create_agent_run_context.rs`). The message of Run 3 had a clause with no Chapter reference after the split at "和", so the resolution was `clarification`. The Context Assembly then selected no working text and declared no target blocks (manifest: `working_target` 0 tokens). Before #1463 this disposition made a clarification question. Now the Run continues: the request has only the author message, and each `propose_edit` call is refused `undeclared_block`. The model used the block identities of the request of Run 2. | The author asked for an edit and got no Proposal after 5 requests. | Reported to the coordinator; no owner issue yet. |
| F2 | After the `propose_edit` call of M12 opened the Proposal, the next RunStep was rebuilt with the Full mapping and no prior response (M13, `prior_continuation_binding_id` null, `known_prior_continuation_binding_id` `01a1d77a-29c9-7841-9d90-6e12875f549d`; the rebuild path of `agent_run_attempt.rs`). M13 sent only the current author message, the working text, the target blocks, the call, and its output: 2,054 input tokens against 12,761 in M12. Its target blocks did not have the changed block. Runs 1 to 3 are thus not in the Provider conversation from M13, also for Run 5. | The model does not read the earlier turns after an edit in a Run. This is related to the known finding that each request after a steering RunStep uses the Full mapping (coordinator note 4). | Reported to the coordinator; no owner issue yet. |
| F3 | Run 1 asked a question, and the model used web research for it with no request of the author. | None: the model decides what a Run gives (ADR 0091 section 6). The plan changed for it. | Observation only. |
