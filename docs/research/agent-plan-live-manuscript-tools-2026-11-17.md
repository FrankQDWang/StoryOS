# Agent Plan live manuscript Tool run (2026-11-17)

This record is the live evidence of [Show the Manuscript Tool Uses in the Panel and Record a Live Run](https://github.com/FrankQDWang/StoryOS/issues/1573) for [ADR 0095](../adr/0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md) section 1. It records three Runs in one Conversation on the real Agent Plan route. The model used the Chapter list, `search_manuscript`, and `read_chapter` to answer questions about Chapters that are not the Working text. The route accepted the strict schema of `read_chapter` with its two nullable integers. The panel showed the line of each search and of each read.

Raw evidence: the wire payload of each request, the Model Attempts, the Context Assemblies, the ToolCalls, the guard log, the plan, and the Server and Worker logs are on the tag `evidence/1573-live-manuscript-tools-2026-11-17`.

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/s7-b2b-manuscript-uses` at `714e44a6a8a5938f068ad75e7915b56b72ab227d`, on `stage7-memory` `0e2681f3`. |
| Release package | `make release-package` on the clean commit above, manifest `sha256:61f2b751bc0f871dfd5fb929b76950604bc94387ed63818c561f0944cbfde8c7`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8750`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with a process environment that has only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout, which it found from its working directory in the worktree. This run never read, copied, or printed that file or a key. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The only direct writes were the guard schema `live_guard`. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`. Each request declared eight Tools: `web_search`, `web_fetch`, `propose_edit`, `submit_research`, `read_tool_result`, `read_chapter`, `search_manuscript`, and `finish`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`, with `localStorage`, `sessionStorage`, and IndexedDB of the origin clear before the first step. The author commands of the panel made the Project 书稿工具实测 `01a1e55f-7e51-7002-b160-f6f5f48e85d6`, the Volume 第一卷, and three Chapters, each with one paragraph typed in the editor. The author then opened 归航, so it was the Current Chapter. |

The three Chapters before Run 1:

| # | Title | Text |
| --- | --- | --- |
| 1 | 听潮 | 林舟十岁那年，外婆把一只旧铜铃交到她手里，说它叫听潮铃。铃舌上刻着一个“潮”字，每回涨潮之前，它会自己响三声。 |
| 2 | 雾港 | 雾港的早晨总是白茫茫的。码头上的老周一边补网，一边告诉林舟，对岸的灯塔已经三年没有亮过了。 |
| 3 | 归航 | 林舟把船系在渡口，抬头看见姐姐林雪站在雨里。 |

The name 听潮铃 occurs only in Chapter 1.

## Plan

Posted on the ticket before the first request: [plan comment](https://github.com/FrankQDWang/StoryOS/issues/1573#issuecomment-6923610201).

- Run 1: the author asks who gave 听潮铃 to 林舟 and what it does. Expected: `search_manuscript` with 听潮铃, then `read_chapter`, then `finish`, and an answer with the fact of Chapter 1.
- Run 2, in the same Conversation: the author names 雾港 by its title and asks what its start says. Expected: `read_chapter` with the number 2, then `finish`.
- Planned: 7 model requests, at most 11 (one more send of a step with a clearer message is in this count). No Exa exchange. About 26,000 reported input tokens. The limit of the ticket is 100,000.
- Request guard: the `live_guard` triggers of the [record of 2026-11-14](agent-plan-live-compaction-2026-11-14.md) with a cap of 11 model requests and 0 Exa exchanges.
- Correction during the run: Run 1 did not call `read_chapter` (finding F1). Inside the cap, the author typed a second paragraph into 听潮 through the editor, opened 归航 again, and sent Run 3 with a question that only the second paragraph answers. Run 3 had 4 requests.

Final count: **10 model requests**, inside the cap of 11. No Exa exchange, no Retrieve, no recovery, and no refusal. Each Model Attempt is `settled`, and each Run is `completed`.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. `prev` is the request whose response the request continued with `previous_response_id`. Each request after M1 sent only its new items (input mapping `incremental`).

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 17:22:18 | 1, 0 | `resp_02179484973898523c5b6bc0dca724893eb7d2b1f969c7c8e8528` | none | required | `search_manuscript` 听潮铃 | 2,630 / 91 |
| M2 | 17:22:21 | 1, 1 | `resp_0217948497416063dcd3c0a2d24a55cb96c7787eab35382f71695` | M1 | required | `finish` | 2,954 / 99 |
| M3 | 17:22:24 | 1, 2 | `resp_021794849744879841fc2298faf73f2fc5d524f9c961e71404a6b` | M2 | none | final answer | 3,099 / 86 |
| M4 | 17:22:58 | 2, 0 | `resp_0217948497786442fe30231c3416749cd01035ba2ea7b61334270` | M3 | required | `read_chapter` 2 | 3,192 / 86 |
| M5 | 17:23:01 | 2, 1 | `resp_02179484978192400000000000000000000ffffac155699e9e910` | M4 | required | `finish` | 3,510 / 34 |
| M6 | 17:23:03 | 2, 2 | `resp_021794849783484593075d5bfc3e0cbf107ff469a4d1c9165c2a5` | M5 | none | final answer | 3,594 / 52 |
| M7 | 17:24:07 | 3, 0 | `resp_0217948498478356863c88f28d563d505635c26ab083df5c9fb80` | M6 | required | `search_manuscript` 听潮铃 | 3,745 / 68 |
| M8 | 17:24:11 | 3, 1 | `resp_021794849851254a4e0bfe9b623ca70f9090a72d521bae2b67c19` | M7 | required | `read_chapter` 1 | 4,043 / 138 |
| M9 | 17:24:15 | 3, 2 | `resp_0217948498558402fe30231c3416749cd01035ba2ea7b61d9abe5` | M8 | required | `finish` | 4,430 / 50 |
| M10 | 17:24:17 | 3, 3 | `resp_02179484985803302e43eb8440d3b95b4166e4756a960114ef5b8` | M9 | none | final answer | 4,530 / 34 |

Total: **10 model requests, 35,727 reported input tokens, and 738 output tokens.** The plan was 7 requests and about 26,000 input tokens. Run 3 added 4 requests and Run 1 used one request less than planned.

RunSteps: Run 1 has 3 (search, `finish`, closing), Run 2 has 3 (read, `finish`, closing), and Run 3 has 4 (search, read, `finish`, closing).

## The section Chapter list as the route got it

The user message of M1. The section is after `Author request` and before `Working text`:

```text
Author request:
听潮铃是谁给林舟的？它在涨潮之前会怎样？请直接在回答里告诉我，不要修改正文。

Chapter list:
{"volume":"第一卷"}
[1,55,"听潮"]
[2,45,"雾港"]
[3,22,"归航","current"]

Working text:
林舟把船系在渡口，抬头看见姐姐林雪站在雨里。
```

M4 had the same list. In M7 the line of Chapter 1 was `[1,76,"听潮"]`, after the second paragraph.

## The strict schema of `read_chapter`

Each request declared `read_chapter` with `"strict": true`, the required arguments `chapter`, `first_block`, and `last_block`, and the type `["integer", "null"]` for `first_block` and `last_block`. The route accepted each of the ten requests with this declaration. The model gave these arguments:

| Request | Arguments |
| --- | --- |
| M4 | `{"chapter": 2, "first_block": 1, "last_block": null}` |
| M8 | `{"chapter": 1, "first_block": 1, "last_block": null}` |

## Each Tool output as the route got it

Each output is the `function_call_output` of its call in the next request, in the `tool_result` boundary, with the escape of the boundary.

M2, the output of the search of M1:

```text
<tool_result source="call_7af5c97u1xpx4nucdm0ddscm" state="ok">{&quot;blocks&quot;:1,&quot;blocks_returned&quot;:1,&quot;complete&quot;:true,&quot;matches&quot;:1,&quot;query&quot;:&quot;听潮铃&quot;}
{&quot;chapter&quot;:1,&quot;chapter_id&quot;:&quot;01a1e55f-f311-76e2-957b-f406f53bd23f&quot;,&quot;title&quot;:&quot;听潮&quot;}
[block 1: 1 match]
林舟十岁那年，外婆把一只旧铜铃交到她手里，说它叫听潮铃。铃舌上刻着一个“潮”字，每回涨潮之前，它会自己响三声。</tool_result>
```

M5, the output of the read of M4:

```text
<tool_result source="call_wigtv3dh9vmefqtgrexfej1d" state="ok">{&quot;blocks&quot;:1,&quot;chapter&quot;:2,&quot;chapter_id&quot;:&quot;01a1e560-6ca7-70c3-9f6a-6f593d9ae3e3&quot;,&quot;first_block&quot;:1,&quot;last_block&quot;:1,&quot;revision_id&quot;:&quot;01a1e560-ac6d-7493-a4ff-9c15d3c068f9&quot;,&quot;title&quot;:&quot;雾港&quot;}
[block 1]
雾港的早晨总是白茫茫的。码头上的老周一边补网，一边告诉林舟，对岸的灯塔已经三年没有亮过了。</tool_result>
```

M8 had the same output as M2 for the search of M7. M9, the output of the read of M8:

```text
<tool_result source="call_daiv345d8fj5w9zplcjsp3y2" state="ok">{&quot;blocks&quot;:2,&quot;chapter&quot;:1,&quot;chapter_id&quot;:&quot;01a1e55f-f311-76e2-957b-f406f53bd23f&quot;,&quot;first_block&quot;:1,&quot;last_block&quot;:2,&quot;revision_id&quot;:&quot;01a1e562-c7e9-7a90-9ad2-5f272e79ccac&quot;,&quot;title&quot;:&quot;听潮&quot;}
[block 1]
林舟十岁那年，外婆把一只旧铜铃交到她手里，说它叫听潮铃。铃舌上刻着一个“潮”字，每回涨潮之前，它会自己响三声。
[block 2]
后来，林舟把它挂在了自己那条小船的船头。</tool_result>
```

## The answers and the panel

| Run | Author request | Answer (summary) | Panel lines |
| --- | --- | --- | --- |
| 1 | 听潮铃是谁给林舟的？它在涨潮之前会怎样？请直接在回答里告诉我，不要修改正文。 | 外婆在林舟十岁那年给了她听潮铃；每回涨潮之前，它会自己响三声。The answer names Chapter 1 《听潮》 as its source. | 在全书中搜索了「听潮铃」 |
| 2 | 《雾港》这一章的开头写了什么？请直接在回答里简短告诉我，不要修改正文。 | The text of the first paragraph of 雾港. | 查阅了章节「雾港」 |
| 3 | 林舟后来把听潮铃放在了哪里？请直接在回答里告诉我，不要修改正文。 | 后来，林舟把听潮铃挂在了自己那条小船的船头。 | 在全书中搜索了「听潮铃」, 查阅了章节「听潮」 |

No Run changed the manuscript. The panel showed the lines of Run 1 and Run 2 in their history turns and the lines of Run 3 in the current turn. `getAgentRun` gave this `manuscript_uses` for each Run:

```json
[{"kind": "keyword_search", "query": "听潮铃"}]
[{"kind": "chapter_read", "chapter_id": "01a1e560-6ca7-70c3-9f6a-6f593d9ae3e3", "title": "雾港"}]
[{"kind": "keyword_search", "query": "听潮铃"}, {"kind": "chapter_read", "chapter_id": "01a1e55f-f311-76e2-957b-f406f53bd23f", "title": "听潮"}]
```

## Key check

The Server log (199 lines), the Worker log (13 lines), the ten wire payloads, the Model Attempts, the ToolCalls, the Context Assemblies, and the guard log have no `Authorization` header, no bearer token, no `x-api-key`, no e-mail address, and no home path.

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| F1 | In Run 1 the model called `search_manuscript` and then `finish`, with no `read_chapter`. Chapter 1 had one Block, so the search result had the whole Chapter and the answer. | Not a defect: the result of a search is the exact text of each Block with a match. Run 3, with a fact in a Block that has no match, shows a search and then a read. | Observation. |
| F2 | For a read of a whole Chapter, the model gave `first_block` 1 and not null (M4, M8). | The Host reads the same Blocks. No change. | Observation. |
| F3 | After the search of M7, the model read the whole Chapter 1, not the Blocks near the match. The Chapter had two Blocks. | No effect at this size. A long Chapter can show if the model uses a range after a search. | Observation for the coordinator. |
