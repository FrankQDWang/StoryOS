# Agent Plan live Memory write run (2026-11-16)

This record is the live evidence of [Write Memory Entries with the Host Tool `write_memory`](https://github.com/FrankQDWang/StoryOS/issues/1576) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) section 5. It records three Runs in three new Conversations on the real Agent Plan route. In Run 1 the model created an entry when the author asked it to remember a fact. In Run 2 the route accepted the section `Memory index`, and the model changed that entry. In Run 3, an ordinary writing request, the model did not call `write_memory`.

Raw evidence: the wire payload of each request, the Model Attempts, the Context Assemblies, the ToolCalls, the Memory Entry versions, the guard log, and the Server and Worker logs are on the tag `evidence/1576-live-memory-write-2026-11-16`.

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/s7-d1a2-write-memory` at `8c601672740f281ed08d4fabeb3a02b779be0fd8` (tree `6a0b0db5`), on `stage7-memory` `87900dd8`. |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:6e8b1ff7126c763cf7bdbad279b2e51209c7414d8ceb20ef338c4738511d75b1`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8750`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with a process environment that has only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout, which it found from its working directory in the worktree. This run never read, copied, or printed that file or a key. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The only direct writes were the guard schema `live_guard`. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, profile `storyos.volcengine-agent-plan.execution.v1`, grant scope `project_writing_assistance_v5`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`, with `localStorage`, `sessionStorage`, and IndexedDB of the origin clear before the first step. New Project 记忆写入实测 `01a1e4f0-6681-7043-80ef-4410994c60d7`, Volume 第一卷, Chapter 渡口 with one paragraph typed in the editor: "雨下了一整夜。林舟站在渡口，等她的姐姐林雪从对岸回来。" No Memory Entry at the start. |

## Plan

Posted on the ticket before the first request: [plan comment](https://github.com/FrankQDWang/StoryOS/issues/1576#issuecomment-6920467653). Planned: 9 model requests (8 expected: 3 + 3 + 2), at most 12, no Exa exchange, about 22,000 reported input tokens. Request guard: the `live_guard` triggers of the [record of 2026-11-14](agent-plan-live-compaction-2026-11-14.md) with a cap of 12 model requests and 0 Exa exchanges.

Final count: **8 model requests, 25,112 reported input tokens, and 2,001 output tokens**, inside each limit. Each first request was about 2,800 tokens, not 2,500: each request declared eight Tools (`web_search` and `web_fetch` too). No Exa exchange, no Retrieve, no recovery, and no refused call. Each Model Attempt is `settled`, and each Run is `completed`.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. Each later request of a Run continued the response before it with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | 15:20:00 | 1, 0 | `resp_0217948424002156216202cdc086c00ba26009437f217e9626ce0` | required | one `write_memory` call, `create` | 2,726 / 264 |
| M2 | 15:20:07 | 1, 1 | `resp_021794842408061e41c82dd1debd95d24831350e3c3a8776c23e2` | required | `finish` | 3,077 / 43 |
| M3 | 15:20:14 | 1, 2 | `resp_0217948424150584d9266badd4680e9e72735e419b86df339fbec` | none | final answer | 3,168 / 81 |
| M4 | 15:24:02 | 2, 0 | `resp_021794842642610b869c66ddd8a6a297966635e3152d6825e524e` | required | one `write_memory` call, `change` of entry 1 | 2,849 / 542 |
| M5 | 15:24:17 | 2, 1 | `resp_02179484265738772ea91cebe75e8e291705b4c0b59095e52f570` | required | `finish` | 3,480 / 51 |
| M6 | 15:24:21 | 2, 2 | `resp_02179484266122093e048b1e87f4ca8485d3a0240faade786ba7b` | none | final answer | 3,578 / 177 |
| M7 | 15:24:49 | 3, 0 | `resp_0217948426893312a2bed297ad64eae2b89e2702342ef9741a618` | required | `finish` | 2,844 / 478 |
| M8 | 15:25:04 | 3, 1 | `resp_021794842704642fe278ef4a49fd99537752324bd80717a6ea376` | none | final answer | 3,390 / 365 |

Each of the eight requests declared `write_memory` and had the paragraph of ADR 0094 section 5 one time in its instructions. M4 to M8 also had the paragraph of the section `Memory index`.

## Run 1: create

Author request: "请记住一个设定：林舟不会游泳，也从不上船，所以她总是在渡口等人。以后写她都要按这个来。这次不要修改正文。"

The arguments of the model in M1: action `create`, type `character`, title `林舟：不游泳、不上船`, description `林舟不会游泳，从不上船，总在渡口等人`, and a text of 65 characters. The call settled with "The Host created the Memory entry 1 with the title "林舟：不游泳、不上船"." It has no Destination Attempt. M2 sent that result as the output of the call in the `tool_result` boundary with the state `ok`. Version 1 of entry 1 has the Memory revision 1, the origin `author_request`, and the entry identity of the ToolCall of the `create`. The answer of M3 names entry 1 and the three points of the fact, and it did not edit the Chapter.

## Run 2: change

The user message of M4 had the section `Memory index` between `Chapter list` and `Working text`:

```text
Memory index:
{"type":"character"}
{"description":"林舟不会游泳，从不上船，总在渡口等人","id":1,"title":"林舟：不游泳、不上船"}
```

Author request: "林舟的设定要改一下：她其实会游泳，小时候常在河里游；只是十年前落水以后，她再也不肯上船。请把你记住的这个设定改过来。这次不要修改正文。"

The route accepted M4. The model called `write_memory` with action `change` and entry `1`, and gave a new title `林舟：会游泳，不肯上船`, a new description, and a new text. It did not create a second entry. Version 2 of entry 1 has the Memory revision 2. M7 of Run 3 got the Index with the changed title and description.

## Run 3: no Memory request

Author request: "帮我看看这一段：林舟在渡口等林雪的这个开头，节奏怎么样？给我两条修改建议，不要修改正文。"

The model called only `finish` in M7, and M8 gave two suggestions. No `write_memory` call. The Project still has two version rows.

## Key check

The Server log (112 lines), the Worker log (11 lines), the eight wire payloads, and the exported rows have no `Authorization` header, no bearer token, no `x-api-key`, no home path, and no database URL.

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| F1 | For an entry of the type `character`, the model wrote the fact into the title (`林舟：不游泳、不上船`), and the `change` of Run 2 changed the title too. The Host compares the type and the title for `memory_entry_exists`, so a later `create` with the title `林舟` or `林舟：会游泳` is not refused. | The rule "one entry for each character" depends on the model alone. A Host sentence such as "the title of a character entry is the name of the character" in the description of the Tool can correct it. | Finding for the coordinator: a change of a Host sentence. |
| F2 | In M4 and M7 the model wrote its reasoning before the call (542 and 478 output tokens). | Output tokens only. | Observation. |
