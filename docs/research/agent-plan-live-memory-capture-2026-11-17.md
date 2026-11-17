# Agent Plan live Memory capture run (2026-11-17)

This record is the live evidence of [Send the Capture Request on the Agent Plan Route and Record a Live Capture](https://github.com/FrankQDWang/StoryOS/issues/1585) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) sections 3, 5, and 8. In one Project Conversation the author sent two messages: a writing habit and an intent for the story that the manuscript does not have. After the two Runs completed, one SQL statement made the schedule row of the Conversation due. The Worker then ran one capture on the real Agent Plan route. The route accepted each capture request. The model read the Chapter, and then it recorded two observations: a `writing_preference` and a `foreshadowing`. The Host validation accepted the list with no refusal.

Raw evidence: the wire payload of each request, the Model Attempts, the ToolCalls, the plan, the request guard and its log, the SQL statement, and the Server and Worker logs are on the tag `evidence/1585-live-memory-capture-2026-11-17`. The tag has no database dump and no Project Export Archive: the context items and the stored rows are in this record only.

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/s7-d3f-capture-live` at `dd315129265f29c645de3ef0e05013c767a3747e` (tree `7092d423`), on `stage7-memory` `749a0bc9`. |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:632c4683d5d9e0778846093ba19523c0de45f8d708943f5978cb17535a746a41`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8750`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with a process environment that has only the named variables, `PATH`, and `HOME`. No `STORYOS_EMBEDDING_DESTINATION`, and no `STORYOS_EXA_API_KEY` in the process environment. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout, which it found from its working directory in the worktree. This run never read, copied, or printed that file or a key. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The direct writes were the guard schema `live_guard` and the one SQL statement of the section [The SQL statement](#the-sql-statement). The Project, the Chapter, its text, and the two messages went through the panel and the editor. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, profile `storyos.volcengine-agent-plan.execution.v1`, grant scope `project_writing_assistance_v5`, output bound 32,768 tokens. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`, with `localStorage` and `sessionStorage` of the origin clear before the first step. Two IndexedDB edit journals of Projects of earlier databases stayed, because their delete did not complete. New Project 记忆捕获实测 `01a1e958-6086-77c1-9b37-0a4dd805f3e5`, Volume 第一卷, Chapter 渡口 with one paragraph typed in the editor: "雨下了一整夜。林舟站在渡口，等她的姐姐林雪把渡船从对岸撑回来。“你又来晚了。”林舟说。林雪没有回答，只把缆绳抛上了岸。" No Memory Entry and no delete mark. One Conversation `01a1e959-6dfb-7130-8ece-32ce0c8c8914` with the default Conversation Memory Settings. |

## Plan

Posted on the ticket before the first request: [plan comment](https://github.com/FrankQDWang/StoryOS/issues/1585#issuecomment-6947194895). Planned: 10 model requests (3 for each Run of the author, 2 to 4 for the capture), at most 14, about 36,000 reported input tokens, at most 200,000. No Exa exchange and no embedding request. Request guard: the `live_guard` triggers of the [record of 2026-11-14](agent-plan-live-compaction-2026-11-14.md), with a third trigger for `embedding_attempts`, counted at the claim: model 14, Exa 0, embedding 0.

A rehearsal on `host_fake` with the same steps came first: two Runs, the SQL statement, and a capture that settled with `no_observation`, with 5 fake requests and no error line.

Two changes to the plan: the Worker setting `STORYOS_LOG` takes only a level, so the first Worker start refused the planned filter and sent nothing. The Worker ran with the default level. Each Run of the author took 2 requests, not 3: the model called `finish` in its first step.

Final count: **6 model requests, 26,087 reported input tokens, and 7,650 output tokens**, inside each limit. The largest input was 6,261 tokens. No Exa exchange, no embedding request, no Retrieve, no recovery, and no refused call. Each Model Attempt is `settled`, and each Run is `completed`.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. `prev` is the request whose response the request continued with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 11:51:54 | 1, 0 | `resp_021794916315002b869c66ddd8a6a297966635e3152d682b475a7` | none | required | `finish` | 3,381 / 1,702 |
| M2 | 11:52:41 | 1, 1 | `resp_02179491636212524e994d08a876ec3f4dc3077f2522d8415ad26` | M1 | none | final answer | 5,153 / 578 |
| M3 | 11:53:21 | 2, 0 | `resp_0217949164015905c7f85291e6ad850e47f0b8bb356de2fa54184` | M2 | required | `finish` | 4,254 / 1,940 |
| M4 | 11:54:14 | 2, 1 | `resp_02179491645506641f89ff67558dec75454d97963e9341bffc2f8` | M3 | none | final answer | 6,261 / 888 |
| M5 | 11:55:00 | capture, 0 | `resp_0217949165005919714750b4c92df19b7d86c8470c87ebf5b68b7` | none | required | one `read_chapter` call | 3,372 / 198 |
| M6 | 11:55:06 | capture, 1 | `resp_021794916506454662416cb134b03a75ced108817244360b81e9a` | none | required | one `record_memory_observations` call | 3,666 / 2,344 |

M1 to M4 declared the eleven Tools of a Run of the author. M5 and M6 declared the four Tools of a capture, each a strict function: `read_chapter`, `search_manuscript`, `read_memory_entry`, and `record_memory_observations`.

## The two Runs of the author

Run 1 `01a1e959-6dfb-7130-8ece-32da5357dda5`, the writing habit: "我写人物对白有个固定习惯：对白里从不用感叹号，人物的情绪全靠动作和停顿来表现。你先看看这一段的对白读起来怎么样，不要修改正文。" The answer of M2 said that the paragraph has no exclamation mark and that the emotion is in the actions and the pauses. It did not edit the Chapter.

Run 2 `01a1e95a-c04e-7cf1-ad43-3109f66be77d`, the intent: "说一下我对后面的打算：林雪其实早就知道父亲落水的真相，但她会一直瞒着林舟，直到最后一卷才说出来。正文里现在还没写到这些。你觉得这个伏笔先在哪里露一点苗头比较好？不要修改正文。" The answer of M4 proposed where to show the first sign, asked the author which truth the story has, and asked whether to write the setting into the Project Memory. The model called no `write_memory`: the author did not ask it to remember (ADR 0094 section 5). No Run wrote a Memory Entry.

## The SQL statement

After the two Runs completed, the schedule row of the Conversation had a due time 30 minutes later. At 11:55:00 this statement made it due:

```sql
UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'conversation_capture' AND conversation_id = '01a1e959-6dfb-7130-8ece-32ce0c8c8914'
```

It changed one row. The Worker claimed the row in the same second and started the capture `01a1e95c-437a-7610-b5d7-ea610387acd9` (Run `01a1e95c-437a-7610-b5d7-ea549d7e607c`, source kind `conversation`, from Run 1 to Run 2). After the capture settled, the schedule row had no due time, and the Worker was stopped. No consolidation pass started on this tip.

## The capture request as the route got it

The Host instructions of M5 and M6 are the text of `capture_instructions` in `crates/storyos-adapter-volcengine-responses/src/capture_wire.rs`, byte for byte. M5 had no author message. Its one user message had two sections:

```text
Conversation messages:
{"role":"author","run_id":"01a1e959-6dfb-7130-8ece-32da5357dda5"}
我写人物对白有个固定习惯：对白里从不用感叹号，人物的情绪全靠动作和停顿来表现。你先看看这一段的对白读起来怎么样，不要修改正文。

{"role":"assistant","run_id":"01a1e959-6dfb-7130-8ece-32da5357dda5"}
**总体判断：读起来是成立的，而且很贴合你的习惯。** …

{"role":"author","run_id":"01a1e95a-c04e-7cf1-ad43-3109f66be77d"}
说一下我对后面的打算：…

{"role":"assistant","run_id":"01a1e95a-c04e-7cf1-ad43-3109f66be77d"}
先说时机，再说具体埋法。…

Chapter list:
{"volume":"第一卷"}
[1,59,"渡口"]
```

The section `Conversation messages` had the four messages of the two Runs, oldest first: the input item `memory_capture_input`, 2,517 tokens, `exact_required`. The Project had no Memory Entry and no delete mark, so the request had no section `Memory index` and no section `Deleted Memory entries`. The Context Assembly of RunStep 0 selected the input item and the `chapter_list` item (28 tokens). RunStep 1 kept them and added the `manuscript_read_result` item of the read (420 tokens).

## Each Tool call of the capture

| RunStep | Call | Arguments | State | Result |
| --- | --- | --- | --- | --- |
| 0 | `read_chapter` | `{"chapter": 1, "first_block": null, "last_block": null}` | settled | the line of Chapter 1 渡口 and `[block 1]` with the exact paragraph |
| 1 | `record_memory_observations` | the list below | settled | "The Host recorded 2 observations, and the capture is complete." |

M6 sent the full input: the user message again, then the `read_chapter` call, then its output in a `tool_result` source tag. It had no `previous_response_id` (finding O1).

## The stored observations

The capture settled with the outcome `observations_recorded`. The table `memory_observations` has two rows, each in the state `waiting`:

| Position | Type | Title | Text |
| --- | --- | --- | --- |
| 1 | `writing_preference` | 对白禁用感叹号 | 作者有固定写作习惯：人物对白中从不使用感叹号，人物情绪主要依靠动作和停顿来表现。后续审读、建议或续写对白时都应遵守这一克制风格。 |
| 2 | `foreshadowing` | 林雪知父亲落水真相 | 作者确定但正文尚未写出的核心伏笔：林雪早就知道父亲落水的真相，却一直瞒着林舟，计划直到最后一卷才说出来。真相的具体机制尚未确定；助手列出的可能包括：她亲眼看见却没救成（愧疚）、知道责任人而选择隐瞒（庇护或恐惧）、或父亲落水一事另有隐情。 |

## Answers to the points of the ticket

- **The route accepts the request form.** M5 and M6 settled: the instructions of a capture, no author message, the sections, four strict functions, and `tool_choice: "required"`.
- **Each response of the capture has a function call.** M5 has one `read_chapter` call, and M6 has one `record_memory_observations` call.
- **The Host validation.** The list of M6 passed with no refusal.
- **RunSteps.** 2, of at most 8.
- **Observations.** 2: one `writing_preference` and one `foreshadowing`, of at most 5. Each text is below 600 characters.
- **The choices of the model.** The model read the Chapter before it recorded, although the input alone was sufficient. The intent became a `foreshadowing`, not an `other`: the author called it a 伏笔. Observation 2 keeps the open truth as uncertain (尚未确定), and it names the possibilities as proposals of the assistant, apart from the decision of the author. Each choice is an expected one. No Host sentence needs a change.

## Key check

The Server log (129 lines), the Worker log (2,266 lines), the refused first Worker start (1 line), the six wire payloads, and the exported rows have no `Authorization` header, no bearer token, no `x-api-key`, no home path, no e-mail address, and no database URL. The two logs have no text of a message, of the input, or of an observation (ADR 0047).

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| O1 | M6, the later request of the capture, continued no prior response. It sent the full input, the call, and the output, and the route accepted it. The incremental form of a later capture request has only the wire test of this ticket. | The input of each later RunStep of a capture is sent again, here about 2,500 tokens. | Observation for the coordinator. |
| O2 | The Project had no Memory Entry and no delete mark, so the live requests had no section `Memory index` and no section `Deleted Memory entries`. | The wire tests of this ticket cover the two sections. | Observation. |
| O3 | M6 had 2,344 output tokens, and M1 and M3 had 1,702 and 1,940: the model wrote its reasoning before the call. | Output tokens only. | Observation. |
| O4 | The Worker logs each idle poll of the passage index work at the level `info`: 2,266 lines in about 6 minutes. `STORYOS_LOG` takes only a level, so a run cannot keep the polls out of the log. | A large Worker log. | Observation for the coordinator. |
