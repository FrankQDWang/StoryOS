# Agent Plan live Chapter capture of an existing manuscript (2026-11-17)

This record is the live evidence of [Capture the Existing Manuscript at the First Completed Run](https://github.com/FrankQDWang/StoryOS/issues/1664) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) section 5. The author typed one Chapter of 790 characters before the first Run of the Project. At its due time the Worker found no completed Run and held the schedule row of the Chapter. Then the author sent one message. When that Run completed, the Worker released the held row, claimed it in the same second, and ran one Chapter capture on the real Agent Plan route. The capture recorded 4 observations, and one consolidation pass made 4 Memory entries from them. No SQL statement made a row due.

Raw evidence: the wire payload of each request, the Model Attempts, the ToolCalls, the plan, the request guard and its log, the read-only query of the schedule rows, and the Server and Worker logs are on the tag `evidence/1664-live-chapter-capture-2026-11-17`. The tag has no database dump and no Project Export Archive: the stored rows are in this record only.

## Setup

| Item | Value |
| --- | --- |
| Product code | `s7-d5b-existing-manuscript` at `da1c01f5` (tree `74a877ae`), on `stage7-memory` `265bab51`. The head has S7-D4c (PR #1670) and the fix of #1671, so a capture that stores observations makes the consolidation row due, and the pass is a model request. The head does not have S7-D6 (PR #1672). S7-D6 changes only the Memory Index line of a flagged entry, and this run has no flag. |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:5ffc5f5491c5b656d62daf58c99a0a51c09bdd42953a6a8bc8ac6dd62dad65b7`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8752`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both came from the release package. Each process environment had only the named variables, `PATH`, and `HOME`. It had no `STORYOS_EMBEDDING_DESTINATION` and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout, which it found from its working directory in the worktree. This run never read, copied, or printed that file or a key. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The only direct write was the guard schema `live_guard`. The Project, the Volume, the Chapter, its text, and the message went through the panel and the editor. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`. Before the first step, `localStorage`, `sessionStorage`, and the IndexedDB edit journal of the rehearsal Project of the origin were clear. New Project 章节捕获实测 `01a1ea65-160a-7a23-82d0-70bef27dfc1c`, Volume 第一卷, Chapter 第一回 `01a1ea66-0cb3-7e32-ab7e-edfb64638b79` with one paragraph typed in the editor: the first 790 characters of `prototypes/chinese-long-form-baseline/public-domain.txt` with its line breaks removed, from "此開卷第一回也" to "利物濟人之德．". The stored text of the authoritative revision `01a1ea66-7a45-7473-85ea-4415517e4327` is equal to that source, byte for byte. No Memory Entry. One Conversation with the default Conversation Memory Settings. |

## Plan

Posted on the ticket before the first request: [plan comment](https://github.com/FrankQDWang/StoryOS/issues/1664#issuecomment-6954269187), confirmed by the coordinator: [confirmation](https://github.com/FrankQDWang/StoryOS/issues/1664#issuecomment-6954354058). Planned: 7 model requests (2 to 3 for the Run of the author, 2 to 8 for the capture, 1 for the pass), at most 12, about 26,000 reported input tokens, at most 150,000. No Exa exchange and no embedding request. Request guard: the `live_guard` triggers of the [record of S7-D3f](agent-plan-live-memory-capture-2026-11-17.md), counted at the claim: model 12, Exa 0, embedding 0.

A rehearsal on `host_fake` with the same steps came first: the Run of the author, the SQL statement, and a Chapter capture that settled with `no_observation`, with 3 fake requests and no error line.

Two changes to the plan:

1. **The row became `held` only at its due time.** A commit of the Chapter text does not write a `held` row. It writes the due time of the rule of Core, 1 hour after the commit. The row becomes `held` when the Worker claims it at that time and finds no completed Run (`no_completed_run`). So that the run shows the release of a `held` row on the packaged Worker with no direct write, the author message waited for that claim, about 1 hour after the commit.
2. **No SQL statement.** The planned `UPDATE` was for a due time in the future. At the release, the rule of Core gave the due time of the commit plus 1 hour, which was already past. So the Worker claimed the row at once, and the `UPDATE` was not necessary. It was not sent.

Final count: **8 model requests, 34,910 reported input tokens, and 6,290 output tokens**, inside each limit. The largest input was 6,957 tokens. No Exa exchange, no embedding request, no Retrieve, and no recovery. Each Model Attempt is `settled`, and each Run is `completed`. Two ToolCalls were refused (finding O1).

## The schedule row of the Chapter

From a read-only query of `storyos.memory_work_schedules` (the query and its three results are `schedule-rows.sql` on the tag):

| Time (UTC) | Event | `held` | Due time |
| --- | --- | --- | --- |
| 16:45:46 | The editor committed the Chapter text. | false | 17:45:46 |
| 17:45:47 | The Worker claimed the row: `no_completed_run`, no request. | true | none |
| 17:46:22 | Read before the author message. | true | none |
| 17:47:36.943 | The Run of the author completed, and the same transaction released the row. | false | 17:45:46, from the rule of Core (no read: the Worker claimed the row before the next read) |
| 17:47:38 | Read after the claim of the Worker at 17:47:36.962: `capture_started`. | false | none |

After the Run of the author completed, the row was not `held`, as the stop condition of the confirmation requires. The schedule row of the Conversation had its due time 30 minutes after the Run, 18:17:36. The Worker was stopped at 17:49:46, after the pass settled. No capture of the Conversation started.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. `prev` is the request whose response the request continued with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 17:46:48 | author, 0 | `resp_021794937608483bdd5db804b0f31a919b8f9d8940b3f7fd6a37b` | none | required | one `read_chapter` call | 4,729 / 388 |
| M2 | 17:46:58 | author, 1 | `resp_021794937618327f0644a9747ad3d38d721f78a849566c68bbd4c` | M1 | required | `finish` | 6,061 / 849 |
| M3 | 17:47:21 | author, 2 | `resp_02179493764132434cf8297fe89ff93b17b5141a0abd40627ea17` | M2 | none | final answer | 6,957 / 597 |
| M4 | 17:47:36 | capture, 0 | `resp_021794937657053276b7ea097d56db2840fe05b4d1a7291bdb265` | none | required | one `read_chapter` call | 2,634 / 244 |
| M5 | 17:47:45 | capture, 1 | `resp_02179493766555127412d2b56652b4ea3ae682b9da5d5c5742150` | none | required | one `record_memory_observations` call, refused | 3,623 / 1,920 |
| M6 | 17:48:45 | capture, 2 | `resp_0217949377252472a2bed297ad64eae2b89e2702342ef97b32120` | none | required | one `record_memory_observations` call, refused | 4,163 / 605 |
| M7 | 17:48:57 | capture, 3 | `resp_021794937738053a4e0bfe9b623ca70f9090a72d521bae2b936d7` | none | required | one `record_memory_observations` call | 4,698 / 593 |
| M8 | 17:49:08 | pass, 0 | `resp_021794937748406841fc2298faf73f2fc5d524f9c961e71cd98af` | none | required | one `apply_memory_operations` call | 2,045 / 1,094 |

M1 to M3 declared the eleven Tools of a Run of the author. M4 to M7 declared the four strict Tools of a capture: `read_chapter`, `search_manuscript`, `read_memory_entry`, and `record_memory_observations`. M8 declared `apply_memory_operations`.

## The Run of the author

Run `01a1ea9e-5837-7931-9c89-cd62bbcfad90`, the message "你先读一下这一回，告诉我里面出现了哪些人物，不要修改正文。" The model read Chapter 1 and then called `finish`. The answer of M3 listed 女媧氏, the stone, and the monk and the Taoist as the figures that act, 甄士隱 and 賈雨村 as names of the preface only, and the narrator and the groups as no specific figures. It did not edit the Chapter: after the run, the head of the Chapter was still the revision that the capture read, and the Project had no Proposal.

## The capture request as the route got it

Capture `01a1ea9f-1679-7f11-9f4a-ce473405ca92` (Run `01a1ea9f-1679-7f11-9f4a-ce333145a7f1`, source kind `chapter`, Chapter revision `01a1ea66-7a45-7473-85ea-4415517e4327`). M4 had no author message. Its one user message had two sections:

```text
Chapter text:
{"chapter_number":1,"chapter_title":"第一回"}

此開卷第一回也．作者自云：因曾歷過一番夢幻之后，… 必有補天濟世之材，利物濟人之德．

Chapter list:
{"volume":"第一卷"}
[1,790,"第一回"]
```

The section `Chapter text` holds the 790 characters as one Block, equal to the stored text: the input item `memory_capture_input`, 834 tokens in the estimate of the Context Assembly, `exact_required`. The Project had no Memory Entry, so the request had no section `Memory index` and no section `Deleted Memory entries`. RunStep 0 selected the input item and the `chapter_list` item (30 tokens). RunStep 1 added the `manuscript_read_result` item of the read (1,187 tokens), and each later RunStep added the `tool_result` item of the refused call before it (237 tokens each).

## Each Tool call of the capture

| RunStep | Call | State | Result |
| --- | --- | --- | --- |
| 0 | `read_chapter` with `{"chapter": 1, "first_block": null, "last_block": null}` | settled | the line of Chapter 1 第一回 and `[block 1]` with the exact paragraph |
| 1 | `record_memory_observations` with 4 observations | refused, `intake_malformed` | "The Host refused this call because its arguments do not agree with the parameters of the Tool, so no result is available, and this is not an empty result." |
| 2 | `record_memory_observations` with the same 4 observations, with small changes in two texts | refused, `intake_malformed` | the same sentence |
| 3 | `record_memory_observations` with the 4 observations | settled | "The Host recorded 4 observations, and the capture is complete." |

The observations of RunSteps 1 and 2 had the fields `kind`, `type`, `title`, and `text`, and they did not have the fields `entry` and `reason`. The strict schema requires each field, and a null value is permitted. The call of RunStep 3 had `"entry": null` and `"reason": null` (finding O1).

## The stored observations and entries

The capture settled with the outcome `observations_recorded`. The table `memory_observations` has 4 rows. After the pass, each is `consumed`:

| Position | Type | Title | Text |
| --- | --- | --- | --- |
| 1 | `writing_preference` | 真假隱喻與夢幻本旨 | 開卷聲明本書將真事隱去（“甄士隱”），借通靈之說、假語村言（“賈雨村”）敷演故事；“夢”“幻”是點醒讀者的關鍵，也是全書立意本旨。作者潦倒後追念昔日女子，認為她們行止見識勝過男子，作書意在使閨閣昭傳。 |
| 2 | `character` | 女媧遺下的通靈頑石 | 女媧在大荒山無稽崖煉成三萬六千五百零一塊頑石，只用三萬六千五百塊，獨遺一塊在青埂峰下。此石經煉後靈性已通，因自己無材補天而日夜悲慚；它質地粗蠢卻已通人性，並渴望人間榮華。 |
| 3 | `major_event` | 石動凡心求入紅塵 | 一僧一道來到青埂峰，談起紅塵榮華富貴，石頭聽後打動凡心，開口自稱“弟子蠢物”，請求二師攜它入世享受榮華。本章在石頭繼續進言時中止，僧人道士是否答應尚未寫明。 |
| 4 | `character` | 神秘一僧一道 | 章首出現的一僧一道骨骼不凡、丰神迥異，能往來青埂峰，談論神仙玄幻與人間繁華；石頭認為他們是仙形道體，必有補天濟世之材。本章未給二人姓名，也未揭示具體來歷。 |

The consolidation row became due when the capture settled. The pass `01a1eaa0-7b49-7741-ab5a-67514d7845c8` (Run `01a1eaa0-7b49-7741-ab5a-674aaeb95d30`) started at 17:49:08 and settled with the outcome `applied`: "The Host applied 4 operations and refused 0, and the pass is complete." It made 4 entries, each `current`, version 1, origin `chapter`, with the source reference of the Chapter:

| Entry | Type | Title | Description |
| --- | --- | --- | --- |
| 1 | `writing_preference` | 真假隱喻與夢幻本旨 | 全書以隱真、假語和夢幻為立意根本，並為閨閣立傳。 |
| 2 | `character` | 女媧遺下的通靈頑石 | 青埂峰下唯一未用於補天的頑石，已通靈性並動凡心。 |
| 3 | `major_event` | 石動凡心求入紅塵 | 頑石聽聞人間繁華後，請求一僧一道帶它入世；結果尚未寫明。 |
| 4 | `character` | 神秘一僧一道 | 兩名來歷未明的僧道人物，能往來青埂峰並談論仙凡之事。 |

## Answers to the points of the ticket

- **A Chapter of an existing manuscript gets a capture at the first completed Run.** The row was `held` before the Run, the completed Run released it, and the Worker started the capture 19 ms later, with no direct write.
- **The capture reads the authoritative text.** The input and the `read_chapter` result have the 790 characters of the head revision, byte for byte.
- **Characters, relationships, and major events.** Two `character` observations and one `major_event`. The model wrote no `relationship`: the paragraph has no relationship between named figures. Observations 3 and 4 keep the open points as open (尚未寫明, 未揭示具體來歷).
- **RunSteps.** 4, of at most 8. **Observations.** 4, of at most 5. Each text is below 600 characters.
- **The pass.** 1 request. Each observation became one new entry with origin `chapter`.

## Key check

The Server log (76 lines), the Worker log (14 lines), the eight wire payloads, and the exported rows have no `Authorization` header, no bearer token, no `x-api-key`, no home path, no e-mail address, and no database URL. The two logs have no text of the Chapter, of the message, or of an observation (ADR 0047).

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| O1 | Two `record_memory_observations` calls did not have the nullable fields `entry` and `reason`, and the Host refused each one as `intake_malformed`. The route did not enforce the strict schema. The retry sentence does not name the missing field, so the model needed two more requests to find it. | 2 more capture requests, about 8,800 input tokens. | Observation for the coordinator. |
| O2 | The input of M4 had the full Chapter text, and the model still called `read_chapter`. The S7-D3f record shows the same choice. | 1 more request and 1,187 tokens in each later RunStep. | Observation. |
| O3 | A commit writes no `held` row. A row becomes `held` only at its due time, 1 hour after the commit, when the Project has no completed Run. A Run that completes before that time finds no `held` row, and the row keeps its due time. | The release path runs only for a Chapter that is older than 1 hour at the first completed Run. | Observation. |
| O4 | M5 had 1,920 output tokens: the model wrote its reasoning before the call. | Output tokens only. | Observation. |
