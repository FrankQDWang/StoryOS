# Agent tool loop changed input live run (2026-11-14)

This record is the live evidence of [An Opened Proposal Removes the Earlier Turns From the Provider Conversation](https://github.com/FrankQDWang/StoryOS/issues/1497) (finding F2 of the [live run of #1464](agent-tool-loop-live-run-2026-11-14.md)), for the specification [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451) and the amendment of [ADR 0085](../adr/0085-reenter-each-settled-tool-result-through-context-assembly.md) section 3. It records two Runs in one Conversation on the real Agent Plan route. Run 1 states a fact in an answer. Run 2 is an edit Run that opens a Proposal, and its final answer must use the fact of Run 1.

## Setup

| Item | Value |
| --- | --- |
| Product code | Branch `claude/1497-incremental-changed-input` at `f665a80e2a3a15da7d4f1f1955c174a9dcc9fe1f` (tree `881c81a9`), from `agent-tool-loop` at `690ce0cf`. |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:4756acb10385f7613745c522191a6af282b2e73ebe551d8faf137828354c87ed`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8744`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with a process environment that has only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout. This run never read, copied, or printed that file or a key. The Server log (138 lines) and the Worker log (7 lines) have no match of `authorization`, `bearer`, or `x-api-key`. In a data dump of the `storyos` schema, the one match of "authorization" is the column `qualification_authorization`. |
| Database | New `scripts/dev-postgres.sh up --interactive` database of this worktree with the controlled fixture. The only direct writes were the guard schema `live_guard`. The Chapter text went through the editor. |
| Request guard | Three triggers of the guard schema: the insert of a Model Attempt counts one model request, and the first `destination_attempt_id` of a ToolCall (on insert or update) counts one Exa exchange. Above the cap the trigger refuses the claim transaction, so nothing goes out. |
| Model | `doubao-seed-2-1-pro-260915`, Model Registration `018f0000-0000-7001-8000-00000000fa19`, mapping `storyos.volcengine-agent-plan.responses.create.v5`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`. `localStorage`, `sessionStorage`, and IndexedDB of the origin were clear before the first step. Project `01a1d7c9-644c-7b82-9470-e18327bb7f0d` ("续接实测"), Chapter 灯塔 `01a1d7ca-0035-7f40-a022-461a0d4574e1` with three paragraphs, Conversation `01a1d7ca-cbba-7d90-82cc-0e1d8a194764`. |

## Plan

Written at 02:00:05 UTC, before the first request.

- Purpose: show on the real route that the requests of an edit Run after an opened Proposal continue the prior response with the current input, and that the final answer uses a fact of the Run before it.
- Run 1, answer (a question with "不要联网"): `finish`, then the closing request. Plan: 2 model requests.
- Run 2, edit in the same Conversation: `propose_edit`, `finish`, then the closing request. Plan: 3 model requests.
- Total plan: 5 model requests and 0 Exa exchanges. Automatic requests (a Retrieve, a compaction request, a correction after a refusal) count.
- Hard cap: 10 model requests and 0 Exa exchanges.

Final count: **5 model requests and 0 Exa exchanges**, equal to the plan. No Retrieve, no compaction request, and no recovery occurred. Each Model Attempt is `settled`.

## Live request log

Each row is one claim that the guard counted. The time is the claim time (UTC) from the Model Attempt identity. The request ID is the complete response reference of the Provider. `prev` is the `previous_response_id` that the request sent. The mapping is the continuation input mapping, and the prior input says if the request sent the current input again (`replaced`). Usage is input / output tokens as the Provider reported them.

| # | Time | Run, RunStep | Request ID | Mapping, prior input | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 02:02:34 | 1, 0 | `resp_0217946217546433dcd3c0a2d24a55cb96c7787eab353824e0f5b` | none, current | none | required | `finish` counted | 1733 / 380 |
| M2 | 02:02:51 | 1, 1 | `resp_0217946217719714e0d847e114bd906533eadfa024bfe6d9edd94` | incremental, current | M1 | none | final answer | 2183 / 315 |
| M3 | 02:03:38 | 2, 0 | `resp_021794621819021dfda270f226adba1bf628b57be6483cffed059` | incremental, current | M2 | required | `propose_edit` settled, Proposal opened | 2404 / 448 |
| M4 | 02:03:50 | 2, 1 | `resp_02179462183106202e43eb8440d3b95b4166e4756a96011d8618c` | incremental, replaced | M3 | required | `finish` counted | 3002 / 186 |
| M5 | 02:04:00 | 2, 2 | `resp_021794621840380de282796fc6f207d159298abcddc43054448d3` | incremental, current | M4 | none | final answer | 3235 / 154 |

Usage total: 12,557 input tokens and 1,483 output tokens in 5 model requests.

## Runs

| Run | Run ID | Author message | Model requests | Tool calls | End | Results |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `01a1d7ca-cbba-7d90-82cc-0e2380386c9b` | "如果把这座灯塔的编号定为 HX-47，建成年份定为 1897 年，放在故事里合理吗？请在回答里复述这个编号与年份。不要联网。" | 2 | `finish` | `completed` | `answer`: it says that the fact is reasonable and repeats "编号 HX-47、建成于 1897 年". |
| 2 | `01a1d7cb-c79f-7bf2-8d5e-42611944619f` | "把本章第二段写得更有画面感：写出灯光扫过海面的样子。只改这一段，不要联网。在最终回答里请告诉我上一轮你复述的灯塔编号与建成年份。" | 3 | `propose_edit` (RunStep 0), `finish` (RunStep 1) | `completed` | `proposal` `01a1d7cb-f518-79e3-a951-583930e5d3a3` (1 location, block `01a1d7ca-52e9-74f0-90b0-bcec69625161`, the second paragraph, `pending`), then `answer` with "上一轮复述的灯塔信息是：编号 HX-47，建成年份 1897 年". |

- The `propose_edit` call of M3 opened the Proposal, so the block of the second paragraph left the declared targets. The current input digest of RunStep 1 (`sha256:ce6e8e7e…`) differs from that of RunStep 0 (`sha256:f3b885ff…`). RunStep 2 has the same digest as RunStep 1.
- M4 is the request of F2. It named M3 as `previous_response_id` and sent two input items: one user message with the current input, then the `function_call_output` of the `propose_edit` call. The message starts with the label paragraph "The input of this author request changed after your last step. This input replaces the earlier input of this author request: read the working text, the references, and the target blocks only from this input." Then it has the author request, the working text of the three paragraphs, and the target blocks of the first and the third paragraph. The route accepted it with HTTP 200.
- M5 named M4 and sent only the `function_call_output` of the `finish` call, because its input did not change.
- Each request of Run 2 after the `propose_edit` call (M4, M5) has a prior response, and the final answer has the fact of Run 1. Before the change, M4 was a full input with no prior response (the M13 row of the #1464 record).
- The Context Assembly records of RunStep 1 and 2 have the author instruction, the working target, and the `tool_result` items. The `sent_content` evidence of each Model Attempt is the exact serialized body.
- The panel showed the answer of Run 1, then for Run 2 the Host sentence "这次修改涉及 1 个章节、1 处文字", the change location, and the answer.

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| G1 | The final answer of Run 2 also says that the second paragraph is no longer in the target blocks, and it asks the author to put it back into the target blocks, or to keep the Proposal. The request of M4 had the target blocks without the second paragraph, because its pending Proposal reserves that block, and no text tells the model the reason. | The author reads a request for an action that does not exist. The Proposal is correct and waits for the decision of the author. | Reported to the coordinator; no owner issue yet. |
