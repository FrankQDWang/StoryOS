# Agent Plan live Memory read run (2026-11-17)

This record is the live evidence of [Read Memory Entries with the Host Tool `read_memory_entry`](https://github.com/FrankQDWang/StoryOS/issues/699) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) section 4. It records two Runs in two new Conversations on the real Agent Plan route. In Run 1 the model created an entry when the author asked it to remember a character, and the title of the entry was only the name of the character. In Run 2 the author asked about two details that only the text of that entry had. The model called `read_memory_entry` with the number of the entry and answered from the text.

Raw evidence: the wire payload of each request, the Model Attempts, the Context Assemblies, the ToolCalls, the Memory Entry versions, the read records, the guard log, and the Server and Worker logs are on the tag `evidence/699-live-memory-read-2026-11-17`.

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/s7-d1b-read-memory-entry` at `c482f970dcb49537a7d17a6899eb63b75abe3fe2` (tree `4425d862`), on `stage7-memory` `f6c6de7a`. |
| Release package | `make generate-persistence-catalog` (which runs `python3 scripts/package-release.py`) on the clean commit above, manifest `sha256:b53070b5dad41cb641f57785e4700cb14143aa9e39264d0cb3004d816e191041`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8750`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with a process environment that has only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout, which it found from its working directory in the worktree. This run never read, copied, or printed that file or a key. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The only direct writes were the guard schema `live_guard`. The Project, the Chapter, and its text went through the panel. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, profile `storyos.volcengine-agent-plan.execution.v1`, grant scope `project_writing_assistance_v5`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`, with `localStorage` and `sessionStorage` of the origin clear before the first step. Two IndexedDB edit journals of Projects of earlier databases stayed, because their delete did not complete. The new Project has its own journal. New Project 记忆读取实测 `01a1e5b3-0f26-7b52-a241-5f71c569c543`, Volume 第一卷, Chapter 渡口 with one paragraph typed in the editor: "雨下了一整夜。林舟站在渡口，等她的姐姐林雪把渡船从对岸撑回来。" No Memory Entry at the start. Each Conversation got the default Conversation Memory Settings, with `use_enabled` true. |

## Plan

Posted on the ticket before the first request: [plan comment](https://github.com/FrankQDWang/StoryOS/issues/699#issuecomment-6925807271). Planned: 6 model requests (3 + 3), at most 9, no Exa exchange, about 20,000 reported input tokens. Request guard: the `live_guard` triggers of the [record of 2026-11-14](agent-plan-live-compaction-2026-11-14.md) with a cap of 9 model requests and 0 Exa exchanges.

Final count: **6 model requests, 22,357 reported input tokens, and 1,156 output tokens**, inside each limit. Each first request was about 3,300 tokens, not 2,900: each request declared ten Tools (`web_search` and `web_fetch` too). No Exa exchange, no Retrieve, no recovery, and no refused call. Each Model Attempt is `settled`, and each Run is `completed`.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC, 2026-11-16; the local date was 2026-11-17). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. Each later request of a Run continued the response before it with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | 18:53:02 | 1, 0 | `resp_021794855183054f0644a9747ad3d38d721f78a849566c66a0c9c` | required | one `write_memory` call, `create` | 3,275 / 586 |
| M2 | 18:53:15 | 1, 1 | `resp_021794855196115295492bf4fae3051b4480808ec74710fe02696` | required | `finish` | 3,939 / 42 |
| M3 | 18:53:18 | 1, 2 | `resp_021794855198392dc6263e9e52ba05a43f20bd75063a4f82d954f` | none | final answer | 4,028 / 150 |
| M4 | 18:53:58 | 2, 0 | `resp_02179485523825283b1843fbf122f4733ac47d5f3a75f9d926b4e` | required | one `read_memory_entry` call, entry 1 | 3,363 / 156 |
| M5 | 18:54:02 | 2, 1 | `resp_0217948552427010736cb0c996dcf6699b9adc2424c5dbc4b3062` | required | `finish` | 3,793 / 115 |
| M6 | 18:54:07 | 2, 2 | `resp_021794855247190f03de131648b6a84481dc8a11850669dda93b7` | none | final answer | 3,959 / 107 |

Each of the six requests declared `write_memory` and `read_memory_entry`. M4 to M6 had the paragraph of the section `Memory index` with the two read sentences of ADR 0094 section 4 ("Call read_memory_entry with the id of an entry when the task needs the text of that entry. …").

## Run 1: create, and the title of the entry

Author request: "请记住林雪的设定：她是渡口的船工，比林舟大六岁。十二岁那年她在船坞被缆绳割伤了左手腕，留下一道旧疤。她把那根缆绳剪下一截，系在船头，绳上打了七个结，每个结代表一个她没能渡过河的人。她从不在船上唱歌，因为母亲是在船上唱着歌落水的。以后写她都要按这个来。这次不要修改正文。"

The arguments of the model in M1: action `create`, type `character`, **title `林雪`**, description `渡口船工，林舟之姐，年长六岁；腕有缆绳旧疤，船头系七结绳，从不在船上唱歌。`, and a text of four list items with each detail. The call settled with "The Host created the Memory entry 1 with the title "林雪"." Version 1 of entry 1 has the Memory revision 1 and the origin `author_request`.

The title is only the name of the character, as the rule of finding F1 of the [write record](agent-plan-live-memory-write-2026-11-16.md) asks. The description names the knots and the rule about songs, but it does not say what a knot means or why she does not sing. Only the text has these two details.

## Run 2: read

The user message of M4 had the section `Memory index` between `Chapter list` and `Working text`:

```text
Memory index:
{"type":"character"}
{"description":"渡口船工，林舟之姐，年长六岁；腕有缆绳旧疤，船头系七结绳，从不在船上唱歌。","id":1,"title":"林雪"}
```

Author request: "林雪船头那截绳子上的每个结代表什么？她为什么从不在船上唱歌？请按你记住的设定回答，不要修改正文。"

The model called `read_memory_entry` with `{"entry": 1}` in M4 and with no text before the call. The call settled with no Destination Attempt. Its result has the lines `type: character`, `title: 林雪`, the description, `origin: author_request`, and `changed_at: 2026-11-16T18:53:15.853Z`, and then the exact text of version 1. The table `memory_entry_reads` has one row: the ToolCall, the Run of Run 2, entry 1, version 1, and the time 18:54:02.

M5 sent the result as the `function_call_output` of the call `call_gpuvux6k4qq0qdy428gnv7xc`. The Context Assembly of M5 selected it with the source class `memory_read_result`, the projection mode `exact_required`, 401 tokens, and the ToolCall identity as its source version. The Operation Requirement recorded the ToolCall with the trust `data_only`. M6 sent the empty output of `finish`.

The answer of M6 gives both details from the text: each knot is one person that she could not take across the river, and she does not sing on the boat because their mother fell into the water from a boat while she sang. It did not edit the Chapter.

## Key check

The Server log (80 lines), the Worker log (8 lines), the six wire payloads, and the exported rows have no `Authorization` header, no bearer token, no `x-api-key`, no home path, and no database URL. The two logs have no title, no description, and no text of an entry (ADR 0047).

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| F1 | Closed: with the rule of S7-D1b in the description of `write_memory`, the model wrote the title `林雪` for a `character` entry, with no fact in it. | The check `memory_entry_exists` on the type and the title now matches a second `create` for the same character. | None. |
| O1 | M1 had 586 output tokens: the model wrote its reasoning before the `write_memory` call. | Output tokens only. | Observation. |
