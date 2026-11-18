# Agent Plan live Memory read run (2026-11-17)

This record is the live evidence of [Read Memory Entries with the Host Tool `read_memory_entry`](https://github.com/FrankQDWang/StoryOS/issues/699) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) section 4. It records two Runs in two new Conversations on the real Agent Plan route. In Run 1 the model created an entry when the author asked it to remember a character, and the title of the entry was only the name of the character. In Run 2 the author asked about two details that only the text of that entry had. The model called `read_memory_entry` with the number of the entry and answered from the text.

Raw evidence: the wire payload of each request, the Model Attempts, the Context Assemblies, the ToolCalls, the Memory Entry versions, the read records, the guard log, and the Server and Worker logs are on the tag `evidence/699-live-memory-read-2026-11-17`.

The section [Search run (S7-D1b2)](#search-run-s7-d1b2) adds two more Runs for [Search Memory Entries with the Host Tool `search_memory`](https://github.com/FrankQDWang/StoryOS/issues/1578), with their own tag `evidence/1578-live-memory-search-2026-11-17`.

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

## Search run (S7-D1b2)

This section records two more Runs in two new Conversations of a new Project, for [#1578](https://github.com/FrankQDWang/StoryOS/issues/1578). In Run 3 the author asked the model to remember two facts, and the model wrote them with `write_memory`. In Run 4 the author asked a question with the phrase "绿色的灯", which only the text of one entry has. The model called `read_memory_entry` and did not search (finding F2). The model called `search_memory` in Run 3, before its writes.

Raw evidence: the tag `evidence/1578-live-memory-search-2026-11-17`, with the same files as the tag of #699 under `docs/research/agent-plan-live-memory-read-2026-11-17/evidence-search/`.

### Setup of the search run

The setup is the setup of the first two Runs, with these changes:

| Item | Value |
| --- | --- |
| Product code | `claude/s7-d1b2-search-memory` at `99f5ac9c539a37289750f746d77e76ee58a227ef` (tree `a6ed612c`), on `stage7-memory` `7dec88e6`. |
| Release package | From the clean commit above, manifest `sha256:f1905593d5ba4eedcb3c04b4ebc3e2387767648151013bec6b8bca5fedd8b6b8`. |
| Database | A new database. The only direct writes were the guard schema `live_guard`. The database was removed after the run. |
| Author surface | New Project 记忆搜索实测 `01a1e5ef-43a5-7a93-9fbe-b1348dafc034`, Volume 第一卷, Chapter 渡口 with the same paragraph. No Memory Entry at the start. The IndexedDB edit journals of Projects of earlier databases stayed in the browser. |
| Tools | Each request declared eleven Tools: `read_memory_entry`, then `search_memory`, then `finish` at the end. |

Plan: [plan comment](https://github.com/FrankQDWang/StoryOS/issues/1578#issuecomment-6927508042), with 6 planned model requests, at most 8, no Exa exchange, and at most 50,000 tokens. Guard cap: 8 model requests and 0 Exa exchanges.

Final count: **7 model requests, 28,273 reported input tokens, and 1,651 output tokens** (29,924 in total), inside each limit. The seventh request is the request after the searches of Run 3. No Exa exchange, no Retrieve, no recovery, and no refused call. Each Model Attempt is `settled`, and each Run is `completed`.

### Request log of the search run

Same columns as the first log. The time is the claim time (UTC, 2026-11-16; the local date was 2026-11-17).

| # | Time | Run, RunStep | Request ID | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | 19:59:30 | 3, 0 | `resp_021794859170590dfda270f226adba1bf628b57be6483cfdfc0e7` | required | two `search_memory` calls, `林雪` and `渡口` | 3,393 / 422 |
| M2 | 19:59:43 | 3, 1 | `resp_021794859183667ace291ce9e318ad358bb23d66c06615214f092` | required | two `write_memory` calls, `create` | 3,948 / 614 |
| M3 | 19:59:58 | 3, 2 | `resp_0217948591985636216202cdc086c00ba26009437f217e9e00fac` | required | `finish` | 4,677 / 45 |
| M4 | 20:00:02 | 3, 3 | `resp_021794859202987f47b3ae95ec9e82c79e9e226fe2407f9579228` | none | final answer | 4,769 / 137 |
| M5 | 20:01:10 | 4, 0 | `resp_0217948592701883b39ea1d457e6524cc696cd2a0824aa14a7c33` | required | one `read_memory_entry` call, entry 2 | 3,537 / 148 |
| M6 | 20:01:15 | 4, 1 | `resp_021794859275377dfda270f226adba1bf628b57be6483cf4b49d7` | required | `finish` | 3,897 / 106 |
| M7 | 20:01:19 | 4, 2 | `resp_0217948592795420736cb0c996dcf6699b9adc2424c5dbccd7702` | none | final answer | 4,052 / 179 |

M5 to M7 had the paragraph of the section `Memory index` with the read sentences and the search sentence "Call search_memory with an exact word when you do not find the entry that you need in the section Memory index." M1 to M4 did not have the paragraph, because the Memory Index of Run 3 had no entry.

### Run 3: search, then write

Author request: "请记住两个设定，分开记成两条。第一条是人物林雪：她是渡口的船工，比林舟大六岁；她随身带着一把黄铜罗盘，罗盘背面刻着父亲的名字“林远山”，她从不让别人碰它。第二条是地点旧渡口：它在河湾的北岸，三十年前发过一次大水，之后渡口搬到了下游；老人们说大水那夜河上有人看见一盏绿色的灯。以后写到这些都要按这个来。这次不要修改正文。"

In M1 the model called `search_memory` two times, with `{"query": "林雪"}` and `{"query": "渡口"}`, to find an entry to change. The Memory was empty, so each call settled with the sentence "No current entry of the Memory has this exact text, and this is not a failure." Each call has a ToolCall record and no Destination Attempt. M2 sent the two results as the `function_call_output` items of the calls, each in the form `<tool_result source="call_…" state="ok">…</tool_result>`. The Context Assembly of M2 selected each one with the source class `memory_search_result`, the projection mode `exact_required`, and the ToolCall identity as its source version. M3 and M4 sent the same two items again. The table `memory_entry_reads` has no row of Run 3.

In M2 the model wrote "两条设定在记忆中都没有现成条目，我将分别新建。" and called `write_memory` two times in one response:

| Entry | Type | Title | Description |
| --- | --- | --- | --- |
| 1 | `character` | 林雪 | 渡口船工，林舟的姐姐，年长六岁；随身黄铜罗盘刻父名林远山，不许人碰 |
| 2 | `other` | 旧渡口 | 位于河湾北岸；三十年前大水后渡口迁往下游，传大水夜河上见绿灯 |

Each text is a list of four items with each detail of the request. The panel showed the answer of M4 and no line for a search.

### Run 4: the question

The words of the request that only one text has, and no title or description: "绿色的灯", "罗盘背面", and "从不让别人碰". The question used "绿色的灯", which only the text of entry 2 has. The description of entry 2 has the shorter word "绿灯".

Author request: "设定里提到过“绿色的灯”吗？它出自哪一条设定，原话是怎么说的？请按你记住的设定回答，不要修改正文。"

The section `Memory index` of M5 had the two entries with their descriptions. The model called `read_memory_entry` with `{"entry": 2}` in M5 and did not call `search_memory`. The call settled with no Destination Attempt, and `memory_entry_reads` has one row: the ToolCall, Run 4, entry 2, version 1. M6 sent the result with the source class `memory_read_result`. The answer of M7 named entry 2 「旧渡口」 and quoted the description line and the exact line of the text: "传闻：老人们说，发大水那一夜，河上有人看见一盏绿色的灯。" It did not edit the Chapter.

### Key check of the search run

The Server log (90 lines), the Worker log (9 lines), the seven wire payloads, and the exported rows have no `Authorization` header, no bearer token, no `x-api-key`, no home path, no e-mail address, and no database URL. The two logs have no search term, no title, no description, and no text of an entry (ADR 0047).

### Findings of the search run

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| F2 | In Run 4 the model read entry 2 directly and did not search. The description of the entry in the Memory Index had "绿灯", so the model found the entry in the index. | The live run shows a search with no match only. The store tests of #1578 show a search with a match. A live search with a match needs a question that no description gives a sign of. | Observation. |
| O2 | In Run 3 the model searched before it wrote, from the description of the Tool only: the requests had no search sentence, because the Memory Index was empty. | One more request in a Run that writes (4, not 3). The search found no entry, so the model created two entries and did not change one. | Observation. |
| O3 | The model wrote the two facts in one response, with two `write_memory` calls. | As planned. | None. |
