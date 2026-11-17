# Agent Plan live Memory consolidation run (2026-11-17)

This record is the live evidence of [Give a Capture the Conflict Item on the Agent Plan Route and Record a Live Pass](https://github.com/FrankQDWang/StoryOS/issues/1587) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) sections 3, 6, 7, and 8. It shows the full path, from two Runs of the author to a Memory Entry that a Run in a new Project Conversation reads:

1. In one Project Conversation the author sent two messages. One gave a writing habit. One gave a decision for the story that the manuscript does not have.
2. One SQL statement made the schedule row of the Conversation due, and the Worker ran one capture. The capture recorded two observations.
3. The settlement of the capture made the consolidation row of the Project due. The Worker then ran one pass on the real Agent Plan route. The route accepted the request of the pass. The reply had one `apply_memory_operations` call with two `create` operations, and the Host applied both.
4. In a new Project Conversation the author asked a question whose answer is the decision. The Memory Index of that Run listed the two new entries. The model read the entry of the decision with `read_memory_entry`, and its answer used the decision.

Raw evidence is on the tag `evidence/1587-live-memory-consolidation-2026-11-17`: the wire payload of each request, the Model Attempts, the ToolCalls, the plan, the request guard and its log, the SQL statement, and the Server and Worker logs. The tag has no database dump and no Project Export Archive. The stored rows are in this record only.

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/s7-d4c-conflict-live` at `05182b8e7b0573cb24a0ea1537c094d16352110c` (tree `4436595e`). This is the merge of `stage7-memory` `92018cfd` (with S7-D4c1, S7-D4b3, and the fix #1671) into the code part of this ticket. The head does not have S7-D5 (PR #1667). |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:67bbce89f24a18f7511a32b22331ded6aaa48f72d17a9d024bc8f4eb47db9b4e`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8750`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both came from the release package. Each process environment had only the named variables, `PATH`, and `HOME`. It had no `STORYOS_EMBEDDING_DESTINATION` and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. This run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The direct writes were the guard schema `live_guard` and the one SQL statement of the section [The SQL statements](#the-sql-statements). The Project, the Chapter, its text, and the three messages went through the panel and the editor. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, output bound 32,768 tokens. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`. Before the first step, `localStorage`, `sessionStorage`, and each IndexedDB database of the origin were clear. New Project 记忆整理实测 `01a1ea2a-edfb-7811-93e4-e971e8d1b6d3`, Volume 第一卷, and Chapter 渡口 with one paragraph typed in the editor: "雨下了一整夜。林舟站在渡口，等她的姐姐林雪把渡船从对岸撑回来。“你又来晚了。”林舟说。林雪没有回答，只把缆绳抛上了岸。" At the start the Project had no Memory Entry and no delete mark. Conversation A `01a1ea2b-cf13-7a01-a2db-3520ce95be66` and Conversation B `01a1ea2f-f2e7-7861-bee5-a019b6840029` had the default Conversation Memory Settings. |

## Plan

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1587#issuecomment-6951166003)), and the coordinator confirmed it ([confirmation](https://github.com/FrankQDWang/StoryOS/issues/1587#issuecomment-6951194235)).

- **Planned:** about 12 model requests and at most 26. The count of 12 came from the measured requests of the [record of S7-D3f](agent-plan-live-memory-capture-2026-11-17.md): each Run of the author used 2 requests, and the capture used 2.
- **Tokens:** about 50,000 reported input tokens and at most 400,000. A stop came for a reported input of one request above 60,000 tokens.
- **Request guard:** the `live_guard` triggers of the [record of 2026-11-14](agent-plan-live-compaction-2026-11-14.md), counted at the claim: model 26, Exa 0, embedding 0.
- **Rehearsal:** the same steps came first on `host_fake`, on the same package. There were two Runs of the author, the SQL statement, a capture, and Run 3 in a new Conversation, with 7 fake requests and no error line. The fake capture records no observation, so no pass started on the fake. The store test `the_request_of_a_pass_has_no_conflict_of_its_pass_row` runs a pass on the fake.

Final count: **10 model requests, 37,737 reported input tokens, and 6,488 output tokens**, inside each limit. The largest input was 5,435 tokens.
- No Exa exchange, no embedding request, no Retrieve, no recovery, and no refused call.
- Each Model Attempt is `settled`, and each Run is `completed`.
- The second SQL statement of the plan was not necessary (see [The SQL statements](#the-sql-statements)).

## Live request log

Each row is one Model Attempt that the guard counted:
- **Time** is the claim time (UTC).
- **Request ID** is the response reference of the Provider.
- **Usage** is the input / output tokens that the Provider reported.
- **prev** is the request whose response the request continued with `previous_response_id`.

| Request | Time | Run, step | Request ID | prev | `tool_choice` | Reply | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 15:41:42 | 1, 0 | `resp_021794930102279fe278ef4a49fd99537752324bd80717a1fa4ce` | none | required | `finish` | 3,379 / 692 |
| M2 | 15:42:09 | 1, 1 | `resp_021794930129331c097066c207cab501b126d22ce27329487aa53` | M1 | none | final answer | 4,140 / 372 |
| M3 | 15:42:51 | 2, 0 | `resp_02179493017172734cf8297fe89ff93b17b5141a0abd406f6c40a` | M2 | required | `finish` | 4,045 / 1,323 |
| M4 | 15:43:39 | 2, 1 | `resp_021794930220099b47388b0cda4b4b6277f5a29d88197f038bd64` | M3 | none | final answer | 5,435 / 1,099 |
| M5 | 15:44:30 | capture, 0 | `resp_021794930270582ea9999fb310f5017a5d432d00d65dc9a3b0792` | none | required | one `read_chapter` call | 3,503 / 326 |
| M6 | 15:44:44 | capture, 1 | `resp_0217949302842779afa3f71968a0bd03105784af777db7b905963` | none | required | one `record_memory_observations` call | 3,796 / 890 |
| M7 | 15:45:14 | pass, 0 | `resp_021794930314623f03de131648b6a84481dc8a11850669dd60203` | none | required | one `apply_memory_operations` call | 1,697 / 706 |
| M8 | 15:46:13 | 3, 0 | `resp_02179493037358883b1843fbf122f4733ac47d5f3a75f9dd6aba4` | none | required | one `read_memory_entry` call | 3,589 / 102 |
| M9 | 15:46:16 | 3, 1 | `resp_021794930377128af77c6bdcb28310871ba6ee41ea156a67bfe15` | M8 | required | `finish` | 3,884 / 335 |
| M10 | 15:46:30 | 3, 2 | `resp_0217949303904796863c88f28d563d505635c26ab083df5390084` | M9 | none | final answer | 4,269 / 643 |

The Tools of each request:
- M1 to M4 and M8 to M10 declared the eleven Tools of a Run of the author.
- M5 and M6 declared the four Tools of a capture.
- M7 declared only `apply_memory_operations`.

Each Tool was a strict function.

## The Runs of the author

- **Run 1** `01a1ea2b-cf13-7a01-a2db-353bc232abff`, the writing habit: "我写人物对白有个固定习惯：对白里从不用感叹号，人物的情绪全靠动作和停顿来表现。你先看看这一段的对白读起来怎么样，不要修改正文。" The answer of M2 discussed the dialogue of the paragraph. It did not edit the Chapter.
- **Run 2** `01a1ea2c-de51-7b61-87e9-405642d14f1d`, the decision: "说一下我对后面的打算：林雪其实早就知道父亲落水的真相，但她会一直瞒着林舟，直到最后一卷才说出来。正文里现在还没写到这些。你觉得这个伏笔先在哪里露一点苗头比较好？不要修改正文。" The answer of M4 proposed where to show the first sign. It asked which truth the story has, and it asked whether to write the setting into the Project Memory. The model called no `write_memory`, because the author did not ask it to remember (ADR 0094 section 5).
- **Run 3** `01a1ea2f-f2e7-7861-bee5-a0211fd2c920`, in Conversation B: "我准备写林雪和林舟的下一场对手戏。按我之前定的安排，林雪对父亲落水这件事知道多少？她在这场戏里该怎么表现？不要修改正文。" See [Run 3 reads the entry](#run-3-reads-the-entry).

After Run 2 the Project had no Memory Entry.

## The SQL statements

After the two Runs completed, the schedule row of Conversation A had a due time 30 minutes later. At 15:44:30 this statement made it due:

```sql
UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'conversation_capture' AND conversation_id = '01a1ea2b-cf13-7a01-a2db-3520ce95be66'
```

It changed one row. The Worker claimed the row in the same second and started the capture `01a1ea2e-60bd-7161-aa52-3e1e06b96e9e` (Run `01a1ea2e-60bd-7161-aa52-3e034063a462`, source kind `conversation`, from Run 1 to Run 2).

The plan had a second statement for the consolidation row, to use only when that row was not due after the capture. It was not necessary. The capture stored observations, so its settlement made the consolidation row due at that time: the Project had no earlier pass (`consolidation_due`, Core). The Worker started the pass `01a1ea2f-0ccd-75a2-b9a6-7dde65e166b2` (Run `01a1ea2f-0ccd-75a2-b9a6-7dc42a788361`) at 15:45:14, and the run wrote no other row directly. After the pass, both schedule rows had no due time.

## The capture

The Host instructions of M5 and M6 are the text of `capture_instructions` in `capture_wire.rs`, byte for byte, with the conflict sentence of this ticket. The Project had no Memory Entry, so the request had no section `Memory index`. The model read the Chapter in M5, and then in M6 it recorded two items, each of the kind `observation`. The Host recorded both with no refusal ("The Host recorded 2 observations, and the capture is complete."). The capture outcome is `observations_recorded`.

| Position | Kind | Type | Title | Text |
| --- | --- | --- | --- | --- |
| 1 | observation | `writing_preference` | 对白不用感叹号 | 作者的固定习惯：人物对白中从不使用感叹号；人物情绪依靠动作、停顿和沉默来表现。后续审稿或改写应保留这种克制风格，不要在台词内加入感叹号。 |
| 2 | observation | `other` | 林雪隐瞒父亲落水真相 | 作者确定：林雪早就知道父亲落水的真相，但会一直瞒着林舟，直到最后一卷才说出来；当前正文尚未写到这些。真相究竟是意外、自尽、被害，还是与林雪有关，以及她隐瞒林舟的具体动机，目前都还没有确定，不能写成既定事实。 |

## The pass request as the route got it

M7 is the one request of the pass. It had:
- the Host instructions of `pass_instructions` in `pass_wire.rs`, byte for byte (2,013 characters);
- no author message and no `previous_response_id`;
- `max_output_tokens` 32,768, `stream` true, `store` true;
- one strict function `apply_memory_operations` with the parameters of the Core declaration;
- `tool_choice: "required"`.

The Project had no entry and no delete mark. So the one user message had only the section `Memory observations`, with one JSON line for each observation:

```text
Memory observations:
{"observation":1,"source":{"conversation_id":"01a1ea2b-cf13-7a01-a2db-3520ce95be66","kind":"conversation"},"text":"作者的固定习惯：人物对白中从不使用感叹号；人物情绪依靠动作、停顿和沉默来表现。后续审稿或改写应保留这种克制风格，不要在台词内加入感叹号。","title":"对白不用感叹号","type":"writing_preference"}
{"observation":2,"source":{"conversation_id":"01a1ea2b-cf13-7a01-a2db-3520ce95be66","kind":"conversation"},"text":"作者确定：林雪早就知道父亲落水的真相，但会一直瞒着林舟，直到最后一卷才说出来；当前正文尚未写到这些。真相究竟是意外、自尽、被害，还是与林雪有关，以及她隐瞒林舟的具体动机，目前都还没有确定，不能写成既定事实。","title":"林雪隐瞒父亲落水真相","type":"other"}
```

The pass row has the same two observations in its input list. The pass pinned the Memory revision 0.

## The operations of the reply

The reply of M7 had one `apply_memory_operations` call. The Host applied both operations and refused none. Its result was: "The Host applied 2 operations and refused 0, and the pass is complete." The pass outcome is `applied`, and both observations are `consumed`.

| Operation | Action | Type | Title | Description | Observations | Result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `create` | `writing_preference` | 对白不用感叹号 | 人物对白中保持克制，不使用感叹号 | 1 | `applied`, entry 1 |
| 2 | `create` | `foreshadowing` | 林雪隐瞒父亲落水真相 | 林雪知道父亲落水真相并瞒住林舟，至最后卷才揭示 | 2 | `applied`, entry 2 |

The text of each operation:
- Entry 1: "作者的固定风格：人物对白中从不使用感叹号。人物情绪应通过动作、停顿和沉默来表现。后续审稿或改写时需保留这种克制，不要在台词内加入感叹号。"
- Entry 2: "林雪早已知道父亲落水的真相，但会一直瞒着林舟，直到最后一卷才说出；当前正文尚未写到这一揭示。父亲落水究竟是意外、自尽、被害，还是与林雪有关，以及林雪隐瞒林舟的具体动机，目前均未确定，不能写成既定事实。"

## The entries before and after

Before the pass the Project had no Memory Entry. After the pass:

| Entry | Version | Memory revision | State | Type | Origin | Title | Cause | Sources |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 1 | 1 | current | `writing_preference` | `conversation` | 对白不用感叹号 | `consolidation` | one `conversation` source |
| 2 | 1 | 2 | current | `foreshadowing` | `conversation` | 林雪隐瞒父亲落水真相 | `consolidation` | one `conversation` source |

The origin of each new entry is `conversation`, because each observation came from the capture of Conversation A. The source of each entry is Conversation A.

## Run 3 reads the entry

The Memory Index of M8 listed the two new entries:

```text
Memory index:
{"type":"writing_preference"}
{"description":"人物对白中保持克制，不使用感叹号","id":1,"title":"对白不用感叹号"}
{"type":"foreshadowing"}
{"description":"林雪知道父亲落水真相并瞒住林舟，至最后卷才揭示","id":2,"title":"林雪隐瞒父亲落水真相"}
```

The Tool calls of Run 3:
- In M8 the model called `read_memory_entry` with `{"entry": 2}`. The Host settled the call with the type, the title, the description, the origin `conversation`, the time of change, and the text of entry 2.
- M9 called `finish`, and M10 gave the final answer.

The answer began with "按你之前定下的安排（记忆：林雪隐瞒父亲落水真相）". It said that 林雪 knows the truth and hides it from 林舟 until the last volume. It also said that the content of the truth and her reason are not decided, so the scene must not make her a culprit. It gave the dialogue advice without exclamation marks. It did not edit the Chapter.

## Answers to the points of the ticket

- **The route accepts the request form.** M7 settled. Its form was the Host instructions of a pass, no author message, the section `Memory observations`, one strict function, `tool_choice: "required"`, and no prior response reference. The route refused no request, so the form of S7-D4c1 needed no correction.
- **The response has one `apply_memory_operations` call.** M7 has one call with two operations.
- **The Host validation.** Each operation passed the validation. No operation was refused.
- **The origin and the source of each new entry.** Each entry has the origin `conversation` and one source, Conversation A.
- **The Memory Index of Run 3.** The Index of M8 lists both new entries.
- **The answer of step 4.** The answer uses the decision of Run 2 after one Memory read call (`read_memory_entry` of entry 2).
- **The conflict item.** The capture request had the conflict sentence and the conflict item in its function. The Project had no entry of the author, so no conflict item was possible in this run. The live run of the conflict path is S7-D6b #1669.

## Key check

The evidence files have no `Authorization` header, no bearer token, no `x-api-key`, no home path, no e-mail address, and no database URL. These files are the Server log (150 lines), the Worker log (17 lines), the ten wire payloads, and the exported rows. The two logs have no text of a message, of the input, of an observation, or of an entry (ADR 0047).

## Findings

| ID | Finding | Effect | Decision |
| --- | --- | --- | --- |
| C1 | The pass gave observation 2 the type `foreshadowing`, but the capture had recorded it as `other`. The capture instructions name `other` for a decision about the story that the manuscript does not have. The pass instructions do not tell the model to keep the type of an observation. The author called the decision a 伏笔 in the message. | The entry is easy to find under `foreshadowing`. The decision and its uncertainty stay in the text. | Observation for the coordinator. It is an expected choice, and no Host sentence needs a change. |
| C2 | The second SQL statement of the plan was not necessary. The first pass of a Project starts at the settlement of the capture. | None. The run wrote one SQL statement only. | Observation. |
| C3 | Run 3 read the entry before it answered, although the Index description held the decision. | One more request (M8), with 3,589 input tokens. | Observation. It is an expected choice: the Index has no text. |

## Open points

- A pass with only conflict items sends no section Memory observations, and the instructions name that section; no live request showed this case.
