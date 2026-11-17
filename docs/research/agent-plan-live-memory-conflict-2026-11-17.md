# Agent Plan live Memory conflict run (2026-11-17)

This record is the live evidence of [Record a Live Conflict between an Entry of the Author and a Chapter](https://github.com/FrankQDWang/StoryOS/issues/1669) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) sections 5, 6, and 7. It shows the full path of a conflict with a Chapter as its source:

1. In Conversation A the author asked the assistant to remember one fact of a character. The model wrote an entry of the author with `write_memory`.
2. The author typed one paragraph in Chapter 渡口 in which that fact is different. One SQL statement made the row of the Chapter due. The Worker ran the Chapter capture, and the capture recorded one conflict item and no fact.
3. The settlement of the capture made the pass due, and the Worker ran the pass at once. The pass request had only the entries. The model gave an empty list, and the Host recorded the flag with the Chapter as its source.
4. In Conversation B the author asked for the next paragraph about that character. The Index line of the entry had the flag. The model read the entry, told the author the conflict, and asked which is correct. It changed no entry.
5. The author answered. The model changed the entry with `write_memory`, and the flag was removed.

Raw evidence is on the tag `evidence/1669-live-memory-conflict-2026-11-17`. It holds the wire payload of each request, the Model Attempts, the ToolCalls, the plan, the request guard and its log, the SQL statement, and the Server and Worker logs. The tag has no database dump and no Project Export Archive. The stored rows are in this record only.

## Setup

| Item | Value |
| --- | --- |
| Product code | `stage7-memory` at `a9fe479cc5323d5ddd3cea8328c2c34bff71bf1d` (tree `ece5d0e5`), with S7-D6, S7-D5, S7-D4c, and the fix #1671. The branch `claude/s7-d6b-live-conflict` adds only a store test and this record. |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:c8e3b67ea6436c2f2cabe0fc340523f57ed733365a7c8779a565ea556071727a`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8760`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both came from the release package. Each process environment had only the named variables, `PATH`, and `HOME`. It had no `STORYOS_EMBEDDING_DESTINATION` and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. This run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh up --interactive` database of this worktree, a separate container from the database of each other live run. The direct writes were the guard schema `live_guard` and the one SQL statement of the section [The SQL statements](#the-sql-statements). The Project, the Chapter, its text, and the three messages went through the panel and the editor. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, profile `storyos.volcengine-agent-plan.execution.v1`, grant scope `project_writing_assistance_v5`, output bound 32,768 tokens. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`. Before the first step, `localStorage`, `sessionStorage`, and each IndexedDB database of the origin were clear. New Project 记忆冲突实测 `01a1ea83-a3fa-7082-83ae-c8ca2bf9849f`, Volume 第一卷, and Chapter 渡口 `01a1ea84-14b3-79c1-9761-84e6a41742a8`, empty until step 2. Conversation A `01a1ea84-5f47-7cd3-a6c4-37c897aa4163` and Conversation B `01a1ea85-b364-76d0-9115-b1508ba6b03c` had the default Conversation Memory Settings. |

## Plan

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1669#issuecomment-6955166597)). The coordinator confirmed it ([confirmation](https://github.com/FrankQDWang/StoryOS/issues/1669#issuecomment-6955186893)).

- **Planned:** 19 model requests and at most 28: about 3 for each Run of the author, at most 8 for the capture, and 1 for the pass. About 12 to 13 were expected.
- **Tokens:** about 70,000 reported input tokens and at most 450,000. A reported input of one request above 60,000 tokens was a stop.
- **Request guard:** the `live_guard` triggers of the [record of 2026-11-14](agent-plan-live-compaction-2026-11-14.md), counted at the claim: model 28, Exa 0, embedding 0.
- **A Conversation capture:** the run prevented it by time. Step 5 had to end within 25 minutes of the completion of step 1, and the Worker stopped when step 5 settled.
- **Rehearsal:** the same steps came first on `host_fake`, on the same package and a separate database. There were three Runs of the author, the SQL statement, and a Chapter capture, with 7 fake requests and no error line. The fake capture records no observation, so no pass started on the fake. The store test `a_chapter_capture_flags_an_entry_of_the_author_and_a_change_removes_the_flag` runs the same path on the fake destination.

Final count: **15 model requests, 76,294 reported input tokens, and 9,319 output tokens**, inside each limit. The largest input was 10,428 tokens.

- No Exa exchange, no embedding request, no Retrieve, and no recovery.
- Each Model Attempt is `settled`, and each Run is `completed`.
- Step 1 completed at 17:18:41, and step 5 settled at 17:24:42. The Worker stopped at 17:24:43. No Conversation capture started.
- The second SQL statement of the plan was not necessary.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. `prev` is the request whose response the request continued with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 17:18:26 | 1, 0 | `resp_021794935906298841fc2298faf73f2fc5d524f9c961e7186f3a4` | none | required | `write_memory` | 3,251 / 253 |
| M2 | 17:18:35 | 1, 1 | `resp_02179493591517091eb505e0b3b4db7bd42120475941dff1c7f33` | M1 | required | `finish` | 3,586 / 24 |
| M3 | 17:18:37 | 1, 2 | `resp_0217949359176224c9833b195a1cac5bbbd18e9e4e5ff83a82ce5` | M2 | none | final answer | 3,661 / 37 |
| M4 | 17:19:12 | capture, 0 | `resp_021794935952099f3d3e58bfdffea9769710a7767b276c35245d2` | none | required | `read_memory_entry` | 1,957 / 100 |
| M5 | 17:19:17 | capture, 1 | `resp_021794935957126039442d358435d172954296e77d251e5a803b5` | none | required | `record_memory_observations` | 2,124 / 217 |
| M6 | 17:19:24 | pass, 0 | `resp_021794935964600af77c6bdcb28310871ba6ee41ea156a6477659` | none | required | `apply_memory_operations` | 1,560 / 67 |
| M7 | 17:19:53 | 2, 0 | `resp_021794935993428b97e702619832bacb4ebf6c3ac1ede6745c50a` | none | required | `read_memory_entry` | 3,623 / 6,119 |
| M8 | 17:22:55 | 2, 1 | `resp_0217949361752339714750b4c92df19b7d86c8470c87ebfb0f21c` | M7 | required | `propose_edit`, refused | 9,936 / 587 |
| M9 | 17:23:08 | 2, 2 | `resp_0217949361887176cb63cadb26a6eb1a5dd8bb71cce3d7f5d27da` | M8 | required | `propose_edit`, refused | 10,316 / 257 |
| M10 | 17:23:14 | 2, 3 | `resp_021794936194554c097066c207cab501b126d22ce273294cd1fa2` | M9 | required | `propose_edit` | 10,428 / 346 |
| M11 | 17:23:23 | 2, 4 | `resp_02179493620357541f89ff67558dec75454d97963e9341b2e2a3d` | M10 | required | `finish` | 4,556 / 253 |
| M12 | 17:23:33 | 2, 5 | `resp_02179493621376436a85968bdbb8a949a5bf709eb7d71bcaf13c7` | M11 | none | final answer | 4,860 / 393 |
| M13 | 17:24:17 | 3, 0 | `resp_021794936258021c097066c207cab501b126d22ce273294be408e` | M12 | required | `write_memory` | 5,092 / 447 |
| M14 | 17:24:29 | 3, 1 | `resp_0217949362694479afa3f71968a0bd03105784af777db7b2c21f2` | M13 | required | `finish` | 5,616 / 63 |
| M15 | 17:24:32 | 3, 2 | `resp_0217949362725189afa3f71968a0bd03105784af777db7bf47023` | M14 | none | final answer | 5,728 / 156 |

Run 1 is `01a1ea84-5f47-7cd3-a6c4-37de1d844736` in Conversation A. Run 2 `01a1ea85-b364-76d0-9115-b16b7532a3e0` and Run 3 `01a1ea89-bd3f-75d2-95b3-22537b4b6a9c` are in Conversation B. The capture is `01a1ea85-127a-7e73-a9ae-70a7539efe5d` (Run `01a1ea85-127a-7e73-a9ae-70954c552521`). The pass is `01a1ea85-433f-7c13-b48f-5c8f0b93154f` (Run `01a1ea85-433f-7c13-b48f-5c781562c440`).

## Step 1: the entry of the author

The message of Run 1 was "请记住：林舟不会游泳，他从小怕水。" M1 called `write_memory` with the action `create`, the type `character`, the title 林舟, the description "林舟不会游泳，并且从小怕水。", and the text "林舟不会游泳；他从小就怕水。涉及水域、渡河、落水或游泳相关情节时，应保持这一设定一致。" The Host created entry 1 with the origin `author_request` at Memory revision 1.

## The SQL statements

After the Chapter text was saved, the row of the Chapter was not `held`, and it was due 3,591 seconds later. At 17:19:11 this statement made it due:

```sql
UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'chapter_capture' AND chapter_id = '01a1ea84-14b3-79c1-9761-84e6a41742a8' AND NOT held
```

It changed one row. The Worker claimed the row in the next second and started the capture. The settlement of the capture made the consolidation row due at once, and the Worker started the pass at 17:19:24. The second statement of the plan was therefore not necessary.

## Step 2: the Chapter capture

The paragraph that the author typed in Chapter 渡口 was "雨停了。林舟脱下外衣，跳进河里，一口气游到了对岸，把漂走的渡船拖了回来。" M4 had no author message. Its user message had three sections:

```text
Chapter text:
{"chapter_number":1,"chapter_title":"渡口"}

雨停了。林舟脱下外衣，跳进河里，一口气游到了对岸，把漂走的渡船拖了回来。

Memory index:
{"type":"character"}
{"description":"林舟不会游泳，并且从小怕水。","id":1,"title":"林舟"}

Chapter list:
{"volume":"第一卷"}
[1,36,"渡口"]
```

M4 and M5 declared the four Tools of a capture. M4 read entry 1. M5 recorded one item, and no fact:

```json
{"observations": [{"kind": "conflict", "entry": 1, "reason": "本章写林舟跳下河游到对岸，与“不会游泳、从小怕水”的设定直接矛盾。", "type": null, "title": null, "text": null}]}
```

The Host recorded it ("The Host recorded 1 observations, and the capture is complete."). The capture outcome is `observations_recorded`.

## Step 3: the pass

The pass request M6 had the instructions of a pass, the one Tool `apply_memory_operations`, and one section:

```text
Memory entries:
{"changed_at":"2026-11-17T17:18:35.019Z","description":"林舟不会游泳，并且从小怕水。","id":1,"origin":"author_request","text":"林舟不会游泳；他从小就怕水。涉及水域、渡河、落水或游泳相关情节时，应保持这一设定一致。","title":"林舟","type":"character"}
```

It had no section `Memory observations` and no section `Deleted Memory entries`. This is the shape of a pass with only a conflict (fix #1671). The model replied with `{"operations": []}`. The Host applied 0 operations, and the pass outcome is `applied`.

Then the Host wrote the flag version of entry 1 with the reason of the conflict item and the source kind `chapter`. The conflict item became `consumed`. The versions of entry 1 after the pass:

| Version | Memory revision | Cause | Flag reason | Flag source | Text |
| --- | --- | --- | --- | --- | --- |
| 1 | 1 | `author_request` | none | none | 林舟不会游泳；他从小就怕水。… |
| 2 | 2 | `conflict_flag` | 本章写林舟跳下河游到对岸，与“不会游泳、从小怕水”的设定直接矛盾。 | `chapter` | the same text as version 1 |

## Step 4: the flag in a Run

The message of Run 2 in Conversation B was "帮我写下一段：林舟在渡口看见一个孩子落水。" The section `Memory index` of M7 was:

```text
{"type":"character"}
{"conflict_flag":true,"description":"林舟不会游泳，并且从小怕水。","id":1,"title":"林舟"}
```

The instructions of M7 had the conflict paragraph of S7-D6 after the paragraph of the Memory Index. M7 called `read_memory_entry` for entry 1. The result had the two lines of the flag:

```text
conflict_flag: 本章写林舟跳下河游到对岸，与“不会游泳、从小怕水”的设定直接矛盾。
conflict_source: Chapter 1 of the Chapter list, 渡口
```

M8 and M9 each called `propose_edit` with an arguments text that is not a JSON object, and the Host refused each call (finding O1). M10 called `propose_edit` with a correct object. It opened one Proposal that adds the next paragraph and stops when 林舟 reaches the water. The final answer of M12 told the conflict and asked the author:

> 有一处设定冲突需要你定夺：记忆档案里记录的是“林舟不会游泳，并且从小怕水”，但工作文本现有一句写他“跳进河里，一口气游到了对岸”，两者直接矛盾（档案里也已标了冲突标记）。……请问以哪个为准？

Run 2 called no `write_memory`. Entry 1 had no new version after Run 2. The Proposal stayed undecided, because no step of the ticket decides it.

## Step 5: the answer of the author

The message of Run 3 in Conversation B was "以第一章为准，他会游泳。请更新这条记忆。" M13 called `write_memory` with the action `change` for entry 1. The new description is "林舟会游泳，水性好（以第一章《渡口》为准）。" The new text says that 林舟 can swim, and that the earlier record is not valid after the confirmation of the author. The Host wrote version 3 at Memory revision 3, with the cause `author_request` and no flag. The final answer of M15 confirmed the change.

## Answers to the points of the ticket

- **The capture or the pass gives a conflict for the entry.** The Chapter capture gave one conflict item for entry 1 with a reason. It gave no fact.
- **The Host records the flag and keeps the text.** Version 2 has the cause `conflict_flag`, the reason, and the source kind `chapter`. Its text is the text of version 1.
- **The request of step 4 has the flag in its Index.** The Index line of M7 has `"conflict_flag":true`. The read result names the reason and "Chapter 1 of the Chapter list, 渡口".
- **The answer of step 4 tells the conflict and asks the author.** It quotes the two facts and asks which is correct. No `write_memory` call changed the entry before the answer.
- **The Run of step 5 changes the entry, and the flag is removed.** Version 3 is the change of the author, with no flag.

## Key check

These files are the evidence of the tag:

- the Server log (188 lines) and the Worker log (22 lines),
- the fifteen wire payloads,
- the exported Model Attempts, ToolCalls, and guard log,
- the plan, the guard SQL, and the SQL statement.

They have no `Authorization` header, no bearer token, no `x-api-key`, no home path, no e-mail address, and no database URL. The two logs have no text of a message, of the Chapter, of an observation, or of an entry (ADR 0047).

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| O1 | In Run 2, M8 and M9 sent `propose_edit` with an arguments text that is not a JSON object: one was a JSON string, and one had a markup fragment. The Host refused each call with `malformed_arguments`, and M10 sent a correct object. | Two more requests, about 20,000 reported input tokens. The Host refusal worked as designed. | Observation for the coordinator. No Host sentence needs a change. |
| O2 | The pass with only a conflict sent only the entries (fix #1671). The pass instructions name the section `Memory observations` and the operation `flag_conflict`, but the request has no observation, so the model cannot flag. The model gave an empty list, and the Host wrote the flag after the pass. | The result is correct. The request costs one call of 1,560 input tokens that can change nothing for the conflict. | Observation for the coordinator. No Host sentence needs a change for correctness. |
| O3 | M7 used 6,119 output tokens and 3 minutes before it called `read_memory_entry`. | One slow step. No limit was near. | Observation. |
| O4 | In Run 2 the model also opened a Proposal for the next paragraph. It stopped the paragraph before the fact of the conflict, so the Proposal is correct for each answer of the author. | The author has one Proposal to decide. | Expected choice. |
