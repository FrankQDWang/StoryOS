# Agent Plan live Project Instruction run (2026-11-16)

This record is the live evidence of [Bind the Project Instruction at `createAgentRun` and Send It in Each Run](https://github.com/FrankQDWang/StoryOS/issues/1574) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) section 2. It records two Runs in one Conversation on the real Agent Plan route. In Run 1 the route accepted the section `Project instruction`, and the answer followed its rule. In Run 2 the author request asked for the opposite of the rule for that answer, and the answer followed the author request (Instruction Precedence).

Raw evidence: the wire payload of each request, the Model Attempts, the Context Assemblies, the ToolCalls, the guard log, and the Server and Worker logs are on the tag `evidence/1574-live-project-instruction-2026-11-16`.

## Setup

| Item | Value |
| --- | --- |
| Product code | `s7-a1b-instruction-binding` at `dc30a926cbde99630d2de9743e53706f939706d7` (tree `ff90925b`), PR #1603 into `stage7-memory`. |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:3c34b89bc57e1e8eca7259a16c6fc724b7fa5dceadde20951a628eab181cd69c`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8749`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with a process environment that has only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout, which it found from its working directory in the worktree. This run never read, copied, or printed that file or a key. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The only direct writes were the guard schema `live_guard`. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, profile `storyos.volcengine-agent-plan.execution.v1`, grant scope `project_writing_assistance_v5`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`, with `localStorage`, `sessionStorage`, and IndexedDB of the origin clear before the first step. New Project 项目指令实测 `01a1e3fc-bbce-7782-855f-aa80b93bfc8a`, Volume 第一卷, Chapter 渡口 with one paragraph typed in the editor: "雨下了一整夜。林舟站在渡口，等她的姐姐林雪从对岸回来。" Conversation `01a1e3fd-e5e8-7fe2-b75c-378b3309eee3`. |
| Project Instruction | The settings page is S7-A2, so a Node script with the generated client sent `updateProjectInstruction` through the public route with `session-a`: Revision 1 `01a1e3fd-a07c-7511-b4bc-a297cd6c36f8`, text `每次回答的最后一行单独写「——你的写作助手」。`. `getProjectInstruction` then gave that Revision. |

## Plan

Posted on the ticket before the first request: [plan comment](https://github.com/FrankQDWang/StoryOS/issues/1574#issuecomment-6914333084).

- Run 1: an author request for a short comment that says nothing about the rule. Run 2, in the same Conversation: an author request that asks, for this answer only, to not write the last line of the rule.
- Planned: 4 model requests (RunStep 0 with `finish`, and the closing request, for each Run), at most 6. No Exa exchange. About 14,000 reported input tokens.
- Request guard: the `live_guard` triggers of the [record of 2026-11-14](agent-plan-live-compaction-2026-11-14.md) with a cap of 6 model requests and 0 Exa exchanges.

Final count: **5 model requests**, inside the limit of 6. Run 2 had one more RunStep (finding F1). No Exa exchange, no Retrieve, no recovery, and no refusal. Each Run is `completed`, and each Run binds Revision 1.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. `prev` is the request whose response the request continued with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 10:53:50 | 1, 0 | `resp_0217948264302352f58812a10f9c6b8f25a472cef1ad3e5598c85` | none | required | `finish` | 1,990 / 130 |
| M2 | 10:53:54 | 1, 1 | `resp_0217948264346928b180e7ab5368500e338034e77cb755da27be4` | M1 | none | final answer | 2,189 / 58 |
| M3 | 10:54:12 | 2, 0 | `resp_0217948264522556e0aa4b8e2b3fc35d377306409bf4419f33b6a` | M2 | required | text, no function call | 2,264 / 73 |
| M4 | 10:54:16 | 2, 1 | `resp_021794826456224ace291ce9e318ad358bb23d66c066152eda464` | M3 | required | `finish` | 2,459 / 81 |
| M5 | 10:54:20 | 2, 2 | `resp_0217948264604950736cb0c996dcf6699b9adc2424c5dbc3d5d39` | M4 | none | final answer | 2,610 / 49 |

Total: **5 model requests, 11,512 reported input tokens, and 391 output tokens.** The plan was 4 requests and about 14,000 input tokens.

## The section as the route got it

The user message of M1 (Run 1, RunStep 0). The section `Project instruction` is after `Author request` and before `Working text`, as the mapping `v8` gives:

```text
Author request:
请为这一段写一句简短的评语，直接在回答里写，不要修改正文。

Project instruction:
每次回答的最后一行单独写「——你的写作助手」。

Working text:
雨下了一整夜。林舟站在渡口，等她的姐姐林雪从对岸回来。

Target blocks:
[block_id: 01a1e3fd-146a-72f1-9c54-bff84e5db0a8]
雨下了一整夜。林舟站在渡口，等她的姐姐林雪从对岸回来。
```

M3 and M4 sent the same section with the author request of Run 2. M2 and M5 sent only the output of the `finish` call, as an incremental request with an unchanged input does. Each decision Context Assembly of the two Runs has the considered `instruction_binding` source with the version `01a1e3fd-a07c-7511-b4bc-a297cd6c36f8` and one selected `instruction_binding` item with the projection mode `exact_required`.

## Run 1: the answer follows the rule

Author request: "请为这一段写一句简短的评语，直接在回答里写，不要修改正文。"

The final answer (M2):

```text
评语：开篇干净，雨声、渡口、等待三笔并置，立刻立起悬念与氛围；若能再点一丝林舟此刻的心情或不安，人物会更可感。

——你的写作助手
```

The last line is the line of the rule. The reasoning summary of M1 says that the answer has "the required closing mark placed on its own line".

## Run 2: the author request wins for this answer

Author request: "这次回答的最后不要写「——你的写作助手」。请再为这一段写一句不同的评语，直接在回答里写，不要修改正文。"

The final answer (M5):

```text
雨景与等待天然带着不安，“从对岸回来”又悄悄埋下未知；句子克制有张力，若让“一整夜”在感官上落一笔，气氛会更逼人。
```

The answer has no line `——你的写作助手`. The reasoning summary of M3 says: "Author's override trumps project rules". This is the Instruction Precedence of the Context contract: the current instruction of the author is above the Project Instruction.

## Key check

The Server log (80 lines), the Worker log (7 lines), the five wire payloads, the other evidence files, and a data dump of the `storyos` and `live_guard` schemas (971 lines) have no `Authorization` header, no bearer token, no `x-api-key`, no database URL, and no Command Challenge secret. The only match of the check is the column `qualification_authorization` in the dump. The dump is not on the tag.

## Findings

- **F1. A text answer before `finish` costs one more request.** In M3 the model wrote the answer as text with no function call, although `tool_choice` was `required`. The Host continued with RunStep 1 (M4), which sent the current input again with the prior reference M3, and the model then called `finish`. This is the tool loop of ADR 0091 section 4, not a change of this ticket. The run stayed inside the planned limit. Owner: none; the coordinator decides if a ticket is necessary.
- **F2. No new Host sentence is necessary.** The model obeyed the rule with no hint in the author request, and it put the author request above the rule with no Host sentence about the rank. The mapping `v8` does not change.
