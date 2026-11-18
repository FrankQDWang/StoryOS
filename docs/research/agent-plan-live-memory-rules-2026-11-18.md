# Agent Plan live run of the Memory rules (2026-11-18)

This record is the live evidence of criterion 8 of [Keep an Entry of the Author and a Conversation Capture to What Was Said, and Return the Observations of a Refused Pass Operation](https://github.com/FrankQDWang/StoryOS/issues/1700) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) sections 5 and 6. The run is an observation of the effect of the new text of the mapping `v9` on the real Agent Plan route.

This record has two runs. The sections from "Findings of the first run" to "What the first run does not show" are the first run, with the first text of `write_memory`. The section [Second run](#second-run) is the run with the changed text of criterion 2, and it states for each finding if the second run changes it.

## Findings of the first run

The route accepted each request, and no stop of the plan occurred. The new text did not give the full effect:

| ID | Finding | Effect |
| --- | --- | --- |
| L1 | **The entry of Part A still holds facts of the manuscript that the author did not say.** The model added a quotation of Chapter 1 and the fact that 赵德明 grew up with 沈怀山. | The new sentences of `write_memory` were in the request, and they did not stop this addition. The observation O1 of the Stage 7 journey stays open on this route. |
| L2 | **In Part B, the model called `write_memory` when the author did not ask it to remember.** The author wrote "我定下一件事", and the model made an entry from the decision. | The entry holds only the words of the author. The call is against the existing rule of the paragraph: "only when the author asks you to remember". |
| L3 | **The capture of Part B recorded no observation.** Entry 2 of L2 was in its Memory Index, and the model read no Chapter. | The capture recorded no fact of the manuscript. This run does not show that a capture records a decision, because the decision was already an entry. |
| L4 | **No pass ran.** A capture with no observation does not make the pass due. | The Host instructions of a pass on the mapping `v9` did not go to the real route. The wire test of the ticket proves their text. |
| L5 | **The capture request was below its range.** Its reported input was 2,281 tokens, and the range of the plan was 3,000 to 8,000. | No effect. The capture read no Chapter, so the request has only one RunStep. |

The ticket states that a finding of this kind does not fail the ticket. After these findings, the author decided one more text and one more run.

Raw evidence is on the tag `evidence/1700-live-memory-rules-2026-11-18`. It holds the plan, the wire payload of each request (`wire/M1.json` to `wire/M7.json`), the Model Attempts, the ToolCalls, the request guard and its log, statement C1 (`due.sql`), the table of each check of the stops (`checks.json`), `measures.json`, `entries.json`, `loader.log`, the run script, and the Server and Worker logs. The tag has no database dump and no Project Export Archive.

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/1700-memory-rules-v9` at `5aa7cb9c90ad3f2b34dcf7f4b503de89e80888e3` (tree `1f133c79eb84e1d2f454055df3b2cece46c35079`). |
| Release package | `python3 scripts/package-release.py` on that clean commit, manifest `sha256:d73239c535124c863585098234d56644c13c06881120345a23d68b6457a53b7e`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` on `127.0.0.1:8792`, and one continuous `storyos-worker`. Both came from the release package with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Each process environment had only the named variables, `PATH`, and `HOME`. It had no embedding destination and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh` database. The direct writes were the guard schema `live_guard` and statement C1. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa23`, mapping `storyos.volcengine-agent-plan.responses.create.v9`. |
| Script | `memory-rules-live.ts`, started by `run.sh` and `rehearse.sh`. The script is the loader of S7-E1b with the steps of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL. |
| Sample | `fixtures/memory-calibration/chapter-01.md`, Chapter 第一章 归, 3,085 characters. The text is original (`PROVENANCE.md`). |

## Plan and final count

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1700#issuecomment-6971895008)). The coordinator confirmed it after a change of criterion 4, and the [head comment](https://github.com/FrankQDWang/StoryOS/issues/1700#issuecomment-6972073877) names the head of the run. A rehearsal of all steps on `host_fake` on the same head came first: 5 fake requests, statement C1 changed 1 row, and no work failed.

| Kind of request | Planned | At most | Actual | Expected input of one request | Actual input of one request |
| --- | --- | --- | --- | --- | --- |
| Run of the author, Part A | 3 | 5 | 3 | 9,000 to 14,000 | 11,059 to 11,874 |
| Run of the author, Part B | 3 | 6 | 3 | 9,000 to 16,000 | 11,406 to 11,997 |
| Capture | 3 | 8 | 1 | 3,000 to 8,000 | 2,281 (finding L5) |
| Pass | 1 | 1 | 0 | 2,500 to 5,000 | none (finding L4) |
| **Total** | **10** | **20** | **7** | about 87,000 tokens in all | **72,330 tokens in all** |

Final count: **7 model requests, 72,330 reported input tokens, and 1,663 output tokens**, inside each limit (20 requests and 200,000 tokens). The request guard counted 7 model requests, no Exa exchange, and no embedding request. Each Model Attempt is `settled`, each Run is `completed`, and no ToolCall was refused.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported.

| # | Time | Run, RunStep | Request ID | Result | Usage |
| --- | --- | --- | --- | --- | --- |
| M1 | 07:28:57 | Part A, 0 | `resp_02179498693747372ea91cebe75e8e291705b4c0b59095e8e069b` | one `write_memory` call | 11,059 / 649 |
| M2 | 07:29:16 | Part A, 1 | `resp_02179498695686700000000000000000000ffffac155880e1579e` | `finish` | 11,788 / 36 |
| M3 | 07:29:23 | Part A, 2 | `resp_0217949869634066863c88f28d563d505635c26ab083df55a6ac6` | final answer | 11,874 / 156 |
| M4 | 07:29:57 | Part B, 0 | `resp_0217949869977241cb6d3ad1bdd7a7436196dd041d7fa42dceb03` | one `write_memory` call | 11,406 / 435 |
| M5 | 07:30:10 | Part B, 1 | `resp_021794987011136dfda270f226adba1bf628b57be6483cf567a32` | `finish` | 11,925 / 24 |
| M6 | 07:30:16 | Part B, 2 | `resp_0217949870169614d9266badd4680e9e72735e419b86df3d83ae1` | final answer | 11,997 / 113 |
| M7 | 07:30:28 | capture, 0 | `resp_0217949870291656fd7a792898c90d4d2724113d22df8fc3efaa4` | one `record_memory_observations` call with an empty list | 2,281 / 250 |

The requests M1 to M6 hold the new sentences of `write_memory`. The request M7 holds the new sentences of a capture of a Project Conversation.

## Part A: the words of the author in an entry

The message of the author in Conversation A (Run `01a1ed8f-0b6d-7123-ac04-1f49facae4de`):

> 请帮我记住一个伏笔：茶馆门板上那张写着“东主有事”的白纸，其实是赵德明替沈怀山写的，这件事后面才揭开。先别改正文。

The model called `write_memory` one time, with the type `foreshadowing`, the title "“东主有事”白纸", and this text:

> 伏笔（作者指定，暂不改正文）：临水茶馆门板上写着“东主有事”的白纸，其实是赵德明替沈怀山（知遥的爷爷）写的，此事后面才揭开。
>
> 第一章已埋的线索：知遥看清纸上是“东主有事”，并注意到“字是毛笔写的，一笔一画很端正。她认得这个字迹，不是爷爷的”。后文揭晓写字人是赵德明（赵师傅，与沈怀山从小一起长大）。

The first paragraph holds what the author said. The second paragraph holds what the author did not say (finding L1):

- The quotation "字是毛笔写的，一笔一画很端正。她认得这个字迹，不是爷爷的" is the text of Chapter 1.
- "赵师傅，与沈怀山从小一起长大" comes from another paragraph of Chapter 1.
- "知遥的爷爷" also comes from the manuscript.

## Part B: a question and a decision in one Conversation

The message of the author in Conversation B (Run `01a1ed8f-f694-7dc0-81b3-2b8090c2ce80`):

> 第一章里，顾青禾是怎么跟知遥说爷爷走时的情形的？请引用正文里的原句回答，不要修改正文。另外我定下一件事：沈明川最后不会卖掉茶馆，他会在第十章的镇上会议上当众撕掉那份意向书。

The Chapter was the working text of the request, so the model called no `read_chapter`. Its answer quoted the sentence of 顾青禾 from Chapter 1. The model also called `write_memory` (finding L2), with the type `major_event`, the title "沈明川拒卖茶馆", and this text:

> 作者确定的后续情节：沈明川最后不会卖掉临水茶馆。他会在第十章的镇上会议上当众撕掉那份卖茶馆的意向书。

This entry adds no fact of the manuscript.

## Statement C1

After the two Runs completed, this statement of the test administrator connection made the capture row of Conversation B due. It changed 1 row:

```sql
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1ed8f-0a2d-71f1-8492-fdb6e6b2b3c1'::uuid AND work_kind = 'conversation_capture' AND source_id = '01a1ed8f-f694-7dc0-81b3-2b7511851828'::uuid
```

## The capture of Conversation B

The Worker started the capture `01a1ed90-71e5-7fe2-88b1-6e099a312430` (Run `01a1ed90-71e5-7fe2-88b1-6dff58ce33e8`). Its one user message had the sections `Conversation messages`, `Memory index`, and `Chapter list`. The Memory Index listed entry 1 and entry 2.

The capture had one RunStep. The model called no read Tool, and it called `record_memory_observations` with an empty list. The capture settled with the outcome `no_observation`.

**The observations of the capture: none.** Finding L3 gives what this shows and what it does not show. No observation waits, so no pass started (finding L4).

## What the first run does not show

- It does not show that the new sentences of `write_memory` change what the model writes: the one entry that could show it has the addition (L1).
- It does not show that a capture of a Conversation records a decision and leaves out a fact of the manuscript when both are available (L3).
- It does not show the Host instructions of a pass on the real route (L4). The return of a refused operation is not a part of this run, because a refusal is an error of the model that a plan cannot cause.

## Second run

The author decided one more text and one more run ("再改一次再试"). The paragraph of `write_memory` on the mapping `v9` got the five rules of the change note of criterion 2. The capture text, the pass text, and the text of `v8` did not change. The plan of the first run applied, with the same Part A, Part B, statement C1, stops, and limits. The [head comment](https://github.com/FrankQDWang/StoryOS/issues/1700#issuecomment-6972520841) names the head of the run.

### Result for each finding of the first run

| ID | Second run | What the second run shows |
| --- | --- | --- |
| L1 | **Changed.** | The entry of Part A holds no fact of the manuscript. The model put the clue of Chapter 1 in its answer and not in the entry. Finding N1 gives what the entry still adds. |
| L2 | **Changed.** | In Part B, the model did not call `write_memory`. |
| L3 | **Changed.** | The capture read Chapter 1 and recorded one observation: the decision of the author. It recorded no quotation and no event of the Chapter. Finding N2 gives what the observation adds. |
| L4 | **Changed.** | One pass ran on the real route with the pass text of `v9`, and it applied one operation. No operation was refused, so no observation had the key `refused`. |
| L5 | **Not changed.** | The first capture request was again below its range. Finding N3 gives the numbers. |

### New findings

| ID | Finding | Effect |
| --- | --- | --- |
| N1 | **The entry of Part A adds two small items that are not facts of the manuscript.** It says that the time of the reveal is not yet set ("具体揭开时机/章节待定"), and it keeps the instruction "先别改正文" of this one request as a lasting line. | The author did not say the first item. The rule "only what the author said" is thus not fully obeyed. |
| N2 | **The observation holds one sentence about the state of the manuscript.** The sentence is "目前手稿只有第一章，此事尚未写入正文，后续章节按此方向处理". The first part comes from the answer of the Agent in the Conversation, and the author did not say it. The name "临水茶馆" also comes from that answer: the author said "茶馆". The pass copied the sentence and the name into entry 2. | The sentence becomes false when the author adds a Chapter. |
| N3 | **Two background requests were below their ranges.** The first capture request reported 2,258 tokens (range 3,000 to 8,000), and the pass reported 1,847 tokens (range 2,500 to 5,000). | No effect. The second capture request reported 5,295 tokens, inside its range. |

This is one run of each message, so it does not show how often the model obeys these rules. There is no third run.

### Setup of the second run

| Item | Value |
| --- | --- |
| Product code | `claude/1700-memory-rules-v9` at `527ced925400b7ce92e107ce46581d08bde57497` (tree `328bcd3dbbc9b7d75e1b382e1f7121e1bf66cb98`). |
| Release package | `python3 scripts/package-release.py` on that clean commit, manifest `sha256:72d5b9f071022c73612391258ba8d5f8162a56301c9acdff978ad8a49397f427`. |
| Rehearsal | All steps on `host_fake` on the same head: 5 fake requests, statement C1 changed 1 row, and no work failed. |
| Topology, settings, model, script, and sample | As in the first run, with a new `scripts/dev-postgres.sh` database. The database was removed after the run. |

Raw evidence is on the tag `evidence/1700-live-memory-rules-run2-2026-11-18`, with the same files as the tag of the first run (`wire/M1.json` to `wire/M8.json` for the requests). The tag has no database dump and no Project Export Archive.

### Plan and final count of the second run

| Kind of request | Planned | At most | Actual | Expected input of one request | Actual input of one request |
| --- | --- | --- | --- | --- | --- |
| Run of the author, Part A | 3 | 5 | 3 | 9,000 to 14,000 | 10,990 to 12,145 |
| Run of the author, Part B | 3 | 6 | 2 | 9,000 to 16,000 | 11,338 to 11,671 |
| Capture | 3 | 8 | 2 | 3,000 to 8,000 | 2,258 and 5,295 (finding N3) |
| Pass | 1 | 1 | 1 | 2,500 to 5,000 | 1,847 (finding N3) |
| **Total** | **10** | **20** | **8** | about 87,000 tokens in all | **67,604 tokens in all** |

Final count: **8 model requests, 67,604 reported input tokens, and 3,494 output tokens**, inside each limit (20 requests and 200,000 tokens). The request guard counted 8 model requests, no Exa exchange, and no embedding request. Each Model Attempt is `settled`, each Run of the author is `completed`, and no ToolCall was refused.

### Live request log of the second run

| # | Time | Run, RunStep | Request ID | Result | Usage |
| --- | --- | --- | --- | --- | --- |
| M1 | 07:53:24 | Part A, 0 | `resp_0217949884045424d9266badd4680e9e72735e419b86df31548ea` | one `write_memory` call | 10,990 / 983 |
| M2 | 07:53:49 | Part A, 1 | `resp_0217949884295244c9833b195a1cac5bbbd18e9e4e5ff8320dbce` | `finish` | 12,060 / 32 |
| M3 | 07:53:54 | Part A, 2 | `resp_021794988434819e0d15f89dc5eb44ad1ad600579228b7917bae7` | final answer | 12,145 / 187 |
| M4 | 07:54:24 | Part B, 0 | `resp_021794988464746ab257ece25bd0fea81c82ca5c15ec8fdc1f2e3` | `finish` | 11,338 / 264 |
| M5 | 07:54:35 | Part B, 1 | `resp_02179498847616393e048b1e87f4ca8485d3a0240faade7a6c2a7` | final answer | 11,671 / 330 |
| M6 | 07:54:50 | capture, 0 | `resp_021794988491026f4d7ae5d724d9e41b76430b6d79530bcbb2ddf` | one `read_chapter` call | 2,258 / 534 |
| M7 | 07:55:08 | capture, 1 | `resp_0217949885088554d7552398ca1982f1d8c06d525f58d82c8db04` | one `record_memory_observations` call | 5,295 / 761 |
| M8 | 07:55:28 | pass, 0 | `resp_021794988529060f4d7ae5d724d9e41b76430b6d79530bc8a5cda` | one `apply_memory_operations` call | 1,847 / 403 |

The requests M1 to M5 hold the five rules of `write_memory`. The requests M6 and M7 hold the sentences of a capture of a Project Conversation. The request M8 holds the sentences about the key `refused`.

### Part A of the second run

The message of the author was the message of the first run (Run `01a1eda5-6dfc-7161-9a29-6ed94d1a17ca`). The model called `write_memory` one time, with the type `foreshadowing`, the title "“东主有事”白纸的伏笔", and this text:

> ## 伏笔
> - 茶馆门板上那张写着“东主有事”的白纸，其实是赵德明替沈怀山写的。
> - 这件事先不点破，留到后面才揭开（具体揭开时机/章节待定）。
> - 作者明确要求：目前不修改正文，只记住此伏笔。

The clue of Chapter 1 that the first run put in the entry is now in the answer of the model:

> 另外提醒一句：第一章里现有的那句“她认得这个字迹，不是爷爷的”正好是这条伏笔很自然的埋设点，将来揭开时可以和赵德明的笔迹呼应上。

### Part B of the second run

The message of the author was the message of the first run (Run `01a1eda6-590c-70a1-904e-6dc79d7646b3`). The only ToolCall of the Run was `finish`. The answer quoted the two sentences of Chapter 1 and repeated the decision of the author. Memory had no new entry after this Run.

### Statement C1 of the second run

This statement changed 1 row:

```sql
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1eda5-6c9e-7af0-9aa5-b9147837fbc4'::uuid AND work_kind = 'conversation_capture' AND source_id = '01a1eda6-590c-70a1-904e-6db4f95fb7e3'::uuid
```

### The capture of the second run

The Worker started the capture `01a1eda6-c023-7f71-8a87-336ac979452a` (Run `01a1eda6-c023-7f71-8a87-335a9e5bb1c9`). The model read Chapter 1 with `read_chapter`, and then it called `record_memory_observations` with one item. The capture settled with the outcome `observations_recorded`.

| Position | Kind | Type | Title | Text |
| --- | --- | --- | --- | --- |
| 1 | `observation` | `other` | 沈明川不会卖掉茶馆 | 作者已定：沈明川最终不会卖掉临水茶馆。他将在第十章的镇上会议上当众撕掉那份意向书。目前手稿只有第一章，此事尚未写入正文，后续章节按此方向处理。 |

### The pass of the second run

The capture made the pass due, and the Worker started the pass `01a1eda7-54aa-7d63-95f3-1cb3b03c8bfb` (Run `01a1eda7-54aa-7d63-95f3-1ca3ca387d37`). Its input had the one observation. It settled with the outcome `applied`, and the observation became `consumed`.

| Operation | Observations | Result | Entry | Type | Title | Text |
| --- | --- | --- | --- | --- | --- | --- |
| `create` | 1 | `applied` | 2 | `major_event` | 沈明川拒卖临水茶馆 | ## 后续剧情方向 / 沈明川最终不会卖掉临水茶馆。 / 他将在第十章的镇上会议上当众撕毁出售意向书。 / 目前手稿仅有第一章，此事尚未写入正文；后续章节按此方向处理。 |

### What the second run does not show

- It does not show how often the model obeys the rules, because each message ran one time.
- It does not show the return of a refused operation on the real route. The store tests prove that rule, because a refusal is an error of the model that a plan cannot cause.
