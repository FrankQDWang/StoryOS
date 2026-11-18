# Stage 7 Memory calibration run (2026-11-17)

This record is the live evidence of [Calibrate the Memory Limits on a Sample Manuscript](https://github.com/FrankQDWang/StoryOS/issues/1590) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) section 8, "Calibration". One run on the real route measured what the Chapter captures, the Conversation captures, and the consolidation passes do on an original sample manuscript of 10 Chapters:

1. The script loaded the 10 Chapters into a new Project. One Run of the author completed in Conversation A. Statement C ran its capture, and one pass ran.
2. Statement A made the row of each Chapter due in Chapter order, each one after the pass of the Chapter before it settled. After each capture, statement B made the pass due. So each Chapter has one capture and one pass.
3. The author sent the three messages of the Conversation script in Conversation A. Statement C ran the capture of the Conversation.
4. The author changed Chapter 3 and Chapter 9. Statement A ran two captures, and two passes ran.
5. In Conversation B, the author asked the three questions. The Worker stopped 1 minute after the last answer.

Raw evidence is on the tag `evidence/1590-memory-calibration-2026-11-17` (commit `e015a347`). It holds the plan, the wire payload of each request (`wire/M1.json` to `wire/M62.json`), the Model Attempts, the ToolCalls, the request guard and its log, `due.sql`, the loader log, `measures.json`, `entries.json` (the entry set after each pass), and the Server and Worker logs. The tag has no database dump and no Project Export Archive.

## Setup

| Item | Value |
| --- | --- |
| Product code | `stage7-memory` at `6944d1fe` with S7-D4c, S7-D5, S7-D5b, S7-D6, and S7-D6b. The branch `claude/s7-d7-memory-calibration` adds the sample, the script, two small changes of shared test helpers, and this record. |
| Release package | `python3 scripts/package-release.py` on the clean commit `5a65d5c4` (tree `e38fa585`), manifest `sha256:cef86daf4a9374d35a85bfd1b86d1702f7c0be1520edc97b9dcec85e8c105c32`. The commit `ee880700` after it changes only the script, which is not in the package. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8750`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, both from the release package. Each process environment had only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh up --interactive` database. The direct writes were the guard schema `live_guard` and the 26 statements of the section [Statements](#statements). |
| Model | `doubao-seed-2-1-pro-260915`, Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`. |
| Script | `node apps/web/test/support/memory-calibration.ts <server> <worker pid> <out dir>`. It sends each command through the HTTP test support and the generated client, at most 8 commands in one minute, and it reads each measure with read-only SQL. It uses no browser. |
| Sample | `fixtures/memory-calibration/`: 10 Chapters (32,248 characters), the two changed Chapters, the key, and the Conversation script. The text is original (`PROVENANCE.md`). |

## Plan and final count

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1590#issuecomment-6959750008)). The coordinator confirmed it ([confirmation](https://github.com/FrankQDWang/StoryOS/issues/1590#issuecomment-6959778100)). A rehearsal of all steps on `host_fake` came first, on the same package and a separate database.

- **Planned:** about 77 to 90 requests and at most 150; about 600,000 reported input tokens and at most 2,500,000. A capture or pass request above 60,000 input tokens, or a request of a Run of the author above 150,000, was a stop.
- **Request guard:** the `live_guard` triggers, counted at the claim: model 150, Exa 0, embedding 0.

Final count: **62 model requests, 818,006 reported input tokens, and 169,328 output tokens**, inside each limit. No stop of the plan occurred.

During the run, the script checked the request count and the input of each request after each capture, each pass, and each Run settled. It did not check the total input, and it did not stop on failed work. The run had no failed work. After review round 1 of the pull request, the script checks each stop of the plan in each poll of a wait, also the total input and failed work (a capture or a pass with the outcome `failed`, or a Run of the author that is `refused` or `cancelled`), and it reads the answers. The run used the earlier version of `ee880700`.

| Kind | Requests | Input | Output | Largest input |
| --- | --- | --- | --- | --- |
| Runs of the author (7) | 22 | 558,286 | 6,780 | 41,595 |
| Captures (14) | 27 | 167,326 | 64,693 | 9,169 |
| Passes (13) | 13 | 92,394 | 97,855 | 10,414 |

- No Exa exchange and no embedding request. Each Model Attempt is `settled`, and each Run is `completed`.
- No capture started by time, and the Worker recorded no rate wait. Each pass had in its input only the observations of the one capture before it.
- The Worker stopped at 21:28:02, 1 minute after the last answer. The row of Conversation B was due at 21:56:57, so no capture of Conversation B started.

## Captures

The time is the claim time (UTC). The time in seconds is from the start of the capture to its last event. Usage is the input / output tokens that the Provider reported. An observation count uses `char` (character), `rel` (relationship), `event` (major event), `fore` (foreshadowing), `other`, and `conflict`.

| # | Source | Start | Time (s) | RunSteps | Requests | Input | Output | Tool calls | Observations | Outcome |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| K1 | Conversation A | 20:27:42 | 49 | 2 | M3–M4 | 2,455 + 5,385 | 2,174 | `read_chapter`, `record_memory_observations` | event 3, rel 2 | `observations_recorded` |
| K2 | Chapter 1 | 20:29:15 | 79 | 2 | M6–M7 | 4,507 + 5,328 | 4,251 | `record_memory_observations` refused `intake_malformed`, `record_memory_observations` | char 3, fore 1, rel 1 | `observations_recorded` |
| K3 | Chapter 2 | 20:34:13 | 86 | 1 | M9 | 4,593 | 3,847 | `record_memory_observations` | fore 2, event 1 | `observations_recorded` |
| K4 | Chapter 3 | 20:37:19 | 77 | 1 | M11 | 4,774 | 3,834 | `record_memory_observations` | char 2, fore 2, event 1 | `observations_recorded` |
| K5 | Chapter 4 | 20:41:54 | 149 | 3 | M13–M15 | 4,804 + 8,947 + 9,169 | 6,618 | `search_manuscript` ×2, `read_memory_entry`, `record_memory_observations` | char 2, fore 1, other 1, rel 1 | `observations_recorded` |
| K6 | Chapter 5 | 20:46:35 | 83 | 1 | M17 | 5,226 | 3,854 | `record_memory_observations` | char 2, fore 1, event 1 | `observations_recorded` |
| K7 | Chapter 6 | 20:50:10 | 86 | 1 | M19 | 5,254 | 4,128 | `record_memory_observations` | char 1, fore 2 | `observations_recorded` |
| K8 | Chapter 7 | 20:55:45 | 127 | 1 | M21 | 5,542 | 6,377 | `record_memory_observations` | char 2, fore 1, event 1, rel 1 | `observations_recorded` |
| K9 | Chapter 8 | 21:01:19 | 82 | 2 | M23–M24 | 5,516 + 7,131 | 3,596 | `read_memory_entry` ×5, `record_memory_observations` | event 1, other 1, rel 2 | `observations_recorded` |
| K10 | Chapter 9 | 21:07:31 | 153 | 4 | M26–M29 | 5,583 + 7,257 + 8,015 + 9,151 | 5,931 | `read_memory_entry` ×12, `record_memory_observations` | char 1, event 1, rel 1 | `observations_recorded` |
| K11 | Chapter 10 | 21:12:38 | 127 | 1 | M31 | 5,767 | 5,773 | `record_memory_observations` | char 2, fore 1 | `observations_recorded` |
| K12 | Conversation A (step 3) | 21:17:15 | 15 | 1 | M43 | 4,710 | 834 | `record_memory_observations` | none | `no_observation` |
| K13 | Chapter 3 (changed) | 21:17:31 | 191 | 5 | M44–M48 | 5,909 + 6,468 + 7,146 + 7,719 + 7,952 | 8,240 | `read_memory_entry` ×4, `search_manuscript` ×2, `record_memory_observations` refused `conflict_entry_unavailable`, `record_memory_observations` | event 1 | `observations_recorded` |
| K14 | Chapter 9 (changed) | 21:21:13 | 110 | 2 | M50–M51 | 5,891 + 7,127 | 5,236 | `read_memory_entry` ×4, `record_memory_observations` | conflict 1 | `observations_recorded` |

- No capture has the outcome `step_limit`. The largest RunStep count is 5 (K13).
- 5 of the 13 captures with observations gave 5 observations, the limit. No call was refused for too many observations or for a too long text.
- K12 recorded nothing, and that is correct: the Runs R2 to R4 of the author had written the three facts of the messages with `write_memory` before the capture.

## Passes

| # | Start | Time (s) | Input captures | Request | Input | Output | Operations | Refusals | Outcome |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P1 | 20:28:31 | 41 | K1 (5 items) | M5 | 2,081 | 2,163 | create 5 | none | `applied` |
| P2 | 20:30:38 | 213 | K2 (5 items) | M8 | 3,026 | 9,963 | update 2, create 7 | none | `applied` |
| P3 | 20:35:41 | 93 | K3 (3 items) | M10 | 3,996 | 4,654 | create 2, update 4 | none | `applied` |
| P4 | 20:38:41 | 191 | K4 (5 items) | M12 | 4,948 | 9,951 | update 3, create 5 | none | `applied` |
| P5 | 20:44:27 | 125 | K5 (5 items) | M16 | 6,047 | 6,911 | create 5, update 3 | none | `applied` |
| P6 | 20:48:02 | 124 | K6 (4 items) | M18 | 6,896 | 6,198 | merge 1, update 1, create 4 | merge `merge_type_mismatch` | `applied` |
| P7 | 20:51:37 | 245 | K7 (3 items) | M20 | 7,664 | 13,226 | create 1, update 6 | none | `applied` |
| P8 | 20:57:53 | 201 | K8 (5 items) | M22 | 8,769 | 11,843 | update 8, create 2 | none | `applied` |
| P9 | 21:02:42 | 285 | K9 (4 items) | M25 | 9,381 | 16,077 | create 1, update 10, delete 3 | none | `applied` |
| P10 | 21:10:07 | 147 | K10 (3 items) | M30 | 9,113 | 7,331 | create 3, update 3 | none | `applied` |
| P11 | 21:14:46 | 39 | K11 (3 items) | M32 | 9,797 | 2,043 | create 1, update 3 | none | `applied` |
| P12 | 21:20:47 | 24 | K13 (1 item) | M49 | 10,414 | 1,054 | update 2 | update `memory_entry_invalid` ×2 | `no_operation` |
| P13 | 21:23:06 | 142 | K14 (1 item) | M52 | 10,262 | 6,441 | merge 1, update 2 | merge `memory_entry_invalid`, update `memory_entry_invalid` ×2 | `applied`, only a conflict |

- Conflict flags: 1 (entry 33, from K14 through P13). Deleted entries: 3 (P9). Merged entries: 0, because the Host refused each of the 2 merges.
- P9 deleted three foreshadowing entries that Chapter 8 resolved: entry 13 (the box and its key), entry 15 (the monthly 200 yuan), and entry 29 (1979 and 1983). Each one gave its facts to other entries first.
- The output of a pass is large: the model gives the full text in each update. P9 gave 16,077 output tokens in 285 seconds.

## Entries after each pass

The index tokens are the counted tokens of the Memory Index item in the first request after the pass.

| Pass | After | Current | By type | By origin | Largest / median length by type (characters) | Index tokens |
| --- | --- | --- | --- | --- | --- | --- |
| P1 | Conversation A, step 1 | 5 | event 3, rel 2 | conversation 5 | event 103/88, rel 81/72 | 400 |
| P2 | Chapter 1 | 12 | char 5, fore 1, event 3, rel 3 | chapter 7, conversation 5 | char 167/149, fore 123/123, event 103/88, rel 137/128 | 1,032 |
| P3 | Chapter 2 | 14 | char 5, fore 2, event 4, rel 3 | chapter 9, conversation 5 | char 213/159, fore 196/159, event 153/95, rel 150/137 | 1,198 |
| P4 | Chapter 3 | 19 | char 7, fore 4, event 4, rel 4 | chapter 14, conversation 5 | char 283/184, fore 196/149, event 223/120, rel 150/132 | 1,641 |
| P5 | Chapter 4 | 24 | char 9, fore 5, event 4, other 1, rel 5 | chapter 19, conversation 5 | char 283/191, fore 196/168, event 223/126, other 138/138, rel 150/132 | 2,078 |
| P6 | Chapter 5 | 28 | char 11, fore 6, event 5, other 1, rel 5 | chapter 23, conversation 5 | char 283/159, fore 196/162, event 223/153, other 138/138, rel 150/132 | 2,401 |
| P7 | Chapter 6 | 29 | char 11, fore 7, event 5, other 1, rel 5 | chapter 24, conversation 5 | char 327/195, fore 255/176, event 223/153, other 138/138, rel 166/137 | 2,548 |
| P8 | Chapter 7 | 31 | char 12, fore 7, event 5, other 1, rel 6 | chapter 26, conversation 5 | char 365/228, fore 255/225, event 347/153, other 138/138, rel 221/158 | 2,686 |
| P9 | Chapter 8 | 29 | char 12, fore 4, event 6, other 1, rel 6 | chapter 24, conversation 5 | char 391/228, fore 255/154, event 361/169, other 138/138, rel 271/224 | 2,566 |
| P10 | Chapter 9 | 32 | char 13, fore 4, event 7, other 1, rel 7 | chapter 27, conversation 5 | char 494/227, fore 255/154, event 361/153, other 138/138, rel 271/223 | 2,809 |
| P11 | Chapter 10 | 33 | char 14, fore 4, event 7, other 1, rel 7 | chapter 28, conversation 5 | char 494/222, fore 255/216, event 361/153, other 138/138, rel 271/223 | 2,903 |
| P12 | Chapter 3 changed | 35 | char 14, fore 4, event 8, other 1, rel 7, pref 1 | author_request 3, chapter 27, conversation 5 | char 494/242, fore 255/216, event 381/169, other 138/138, rel 271/223, pref 122/122 | 3,155 |
| P13 | Chapter 9 changed | 35 | char 14, fore 4, event 8, other 1, rel 7, pref 1 | author_request 3, chapter 27, conversation 5 | char 494/242, fore 255/216, event 381/169, other 138/138, rel 271/223, pref 122/122 | 3,176 |

The snapshot of P12 also has the three entries that R2 to R4 wrote: R2 changed entry 33 (沈知南), so its origin became `author_request`, and R3 and R4 created entries 37 and 38. The Chapter list item has 187 to 197 counted tokens in each request.

## Against the key

The final set has 35 current entries and 7,700 characters of text.

- **Characters:** each of the 13 characters of the key has an entry or is in the text of an entry. Entries 31 (顾远) and 36 (何振东) are characters that the key names only in a line. The ages of the key are not in the text of the Chapters, so no entry has them.
- **Relationships:** each of R1 to R8 is in an entry.
- **Foreshadowing:** F1 to F4 are each in an entry. F1 and F2 moved from their foreshadowing entries into entries 32, 30, and 31 when P9 deleted entries 13 and 15. F3 is entry 22, and F4 is entry 25.
- **Facts that a later Chapter changes:** C1 (1979, not 1983) is in entries 7, 9, 17, 18, and 30. C2 (the letter of intent) is in entries 2, 4, 6, and 8.
- **Major events:** each Chapter has its event in an entry.
- **The change of Chapter 3 has no entry.** Entries 2 and 16 still say that 程立 offered 800,000 yuan at his first visit. The cause is in the section [Findings](#findings), F1.
- **The change of Chapter 9** has the conflict flag on entry 33, and the text of entry 33 stays as the author asked.
- **Small details:** 9 of the 24 small details are in the text of an entry: the yellow umbrella (4), the silver watch (5), the red bicycle (11), the bamboo pipe (12), the gold letters on black wood (14), the name 《青溪小报》 (16), the glasses with a black frame (20), the seven arches (21), and the second grade (24). No entry holds only a small detail. Entry 26 also has the price list of the teas, which the key does not name.

## The three questions

The Index of each request had 35 entries, 3,176 counted tokens, and the flag of entry 33.

| Question | Run | Reads | Answer | Fact of the key |
| --- | --- | --- | --- | --- |
| 茶馆当年买房子的钱是谁出的？ | R5, 5 RunSteps | `search_manuscript` ×2, `read_chapter` (Chapter 8), `read_memory_entry` 33 | 沈怀山 and 顾青禾 half each, 2,100 of 4,200 yuan. It also asks which fact of 沈知南 is correct. | yes |
| 沈知南现在在哪里读书，学什么？ | R6, 2 RunSteps | none | It gives the text of Chapter 9 and the entry of the author, says that they conflict, and asks the author to decide. | yes |
| 我之前说顾青禾后面会怎么样？ | R7, 3 RunSteps | `read_memory_entry` 38 | She dies in Chapter 12 and leaves her half of the house to 陆小满. | yes |

The answers are the `decision.text` of the last request of each Run (M57, M59, M62). The executor read them with a read-only query before the database was removed (section [Queries](#queries)). The text is as the model gave it:

> **R5:** 当年买房的钱是**沈怀山与顾青禾各出一半**：一九八三年房管所卖临水街一号，总价四千二百元；沈怀山钱不够，顾青禾拿出亡夫留下的抚恤金**两千一百元**凑齐半价。沈怀山立字据写明房屋二人共有、各占一半，变卖须双方同意，赵德明见证；二〇一九年又重写字据并经镇司法所公证。因当年忌讳寡妇与鳏夫联名，房契上只写了沈怀山一人。
>
> 另待您确认：沈知南的设定以**北京建筑学大四（测绘）**为准，还是以第九章现写的**成都临床医学大四**为准？

> **R6:** 按**正文第九章现在的写法**，沈知南在**成都**读大学，学的是**临床医学**，今年大四（证据：他带着《内科学》，说“学医就是这样，放假也得背”，还在本子上画心脏血管、给顾青禾量血压）。
>
> 但这与您此前定下的固定设定冲突：记忆条目33写的是他在**北京**读**建筑学**大四，回乡是为了测绘茶馆和老桥（该条目已标冲突）。
>
> 所以两处目前不一致。请您裁定以哪个为准：
> - 若以**北京建筑学**为准，我可以把第九章的《内科学》、学医台词、画血管、量血压等细节改成测绘相关内容；
> - 若以**成都临床医学**为准，我就更新记忆条目33，后续章节按医学生续写。
>
> 您说一声即可，我再动笔。

> **R7:** 根据您此前留下的剧情备忘（记忆条目38，来源是您本人的要求，正文目前只写到第十章，尚未落实）：
>
> **顾青禾将在第十二章去世**，并在生前立下遗嘱，把自己名下**那一半临水茶馆房产留给陆小满**（陆砚的高中生妹妹），而不是亲子顾远。
>
> 当时记下的几个牵连点：
> - 她这一半产权易主后，知遥未来的共有人、也是卖房否决权人，就变成了陆小满，直接牵动保馆与收购主线。
> - 她有亲儿子顾远（深圳工程师），却把房产留给非亲非故的陆小满，这个“越过亲生子的托付”需要在前文铺垫动机，也可能引发顾远的反应或冲突。
> - 陆小满当时还是高中生、未成年即承接半栋房产，她本人的意愿、监护问题以及陆砚的态度，都可能成为情节点。
>
> 这只是规划备忘，不是已成文的事实；写到第十一、十二章时以您最新的指示为准。需要的话，我可以帮您在现有章节里检查、补一些为这份遗嘱做铺垫的伏笔。

The Runs of the author in Conversation A had 2, 4, 3, and 3 RunSteps. The input of one request grew from 10,738 (R1) to 41,595 (R4), because each Run carries the earlier turns of the Conversation.

## The three counts of the plan

- **Passes with only a conflict item (finding O2 of S7-D6b):** 1, P13. Its request had 10,262 input and 6,441 output tokens. In S7-D6b the same kind of pass had 1,560 / 67 and an empty list. Here the model gave 3 operations that no observation supports, and the Host refused each one.
- **Refusals `intake_malformed` of a capture (finding O1 of S7-D5b):** 1, in K2 (M6). Each observation of the call had no key `entry` and no key `reason`. The retry M7 cost 5,328 input tokens.
- **`read_chapter` calls of a Chapter capture:** 0. The one `read_chapter` call of a capture is in K1, the Conversation capture, which has no Chapter text in its input.

## Defaults

- **1. At most 300 entries: kept.** 10 Chapters gave 35 entries, and the Index item had 3,176 tokens, about 90 tokens for each entry. At 300 entries the Index item is about 27,000 tokens in each request of a Run and of a capture. The input of a pass grew by about 300 tokens for each entry (2,081 to 10,414), so a pass at 300 entries has about 90,000 input tokens. This is inside the window of the route. The run did not reach the limit.
- **2. At most 600 characters, 1,500 for a character: kept.** The largest text was 494 characters (a character) and 381 for the other types. No call was refused for a too long text.
- **3. At most 5 observations in one capture: kept.** 5 of 13 captures used the full limit, and no call gave more. The entries already hold 9 of the 24 small details, so a higher limit is likely to add details and not facts that a later Chapter needs.
- **4. A Conversation capture after 30 minutes: kept, not measured.** Statement C started each Conversation capture. One capture took 15 to 49 seconds and 1 to 2 requests.
- **5. A Chapter capture after 1 hour, one in 6 hours: kept, not measured.** Statement A started each Chapter capture. One capture took 77 to 191 seconds, 1 to 5 requests, and 4,593 to 35,194 input tokens in all.
- **6. At most one pass in 30 minutes: kept.** Statement B made 12 of the 13 passes due. One pass took 24 to 285 seconds, which is much less than 30 minutes. The run gives no measure against a shorter interval.
- **7. At most one Chapter capture in 2 minutes: kept, not measured.** The script sent statement A only after the pass before it settled, so no rate wait occurred.
- **8. 90 days without a read: kept, not measured.** The run is shorter than 90 days.
- **16. At most 8 RunSteps in one capture: kept.** The largest count was 5 (K13). No capture has the outcome `step_limit`.

No value changes. The findings below are for the coordinator. This ticket does not change the instructions.

## Findings

- **F1. A pass update with no title (M49, M52).** The Host admits an update of a pass only with a title, a description, and a text, and the strict schema lets the model send `null` for each key. In P12 and P13, each of the 5 updates and merges had `"title": null`, and the Host refused each one with `memory_entry_invalid`. A pass has one call, so the model had no retry. Because of this, the change of Chapter 3 has no entry. In P1 to P11 each update had a title.
- **F2. The capture shape of O1 again (M6).** K2 sent observations with no key `entry` and no key `reason`. The retry text does not name the key. This is the second record of O1.
- **F3. A Conversation capture records manuscript facts (M3, M4).** K1 read Chapter 10 because the answer of R1 named its events, and it recorded 5 facts of the manuscript with the origin `conversation`. The sentence that the model did not obey: "A Memory entry is for a fact that a later Chapter needs and that a search of the manuscript cannot find."
- **F4. Small details (M7 to M31).** 9 of the 24 small details of the key are in entries. The sentence that the model did not obey: "A Memory entry is for a fact that a later Chapter needs."
- **F5. A conflict with an entry that is not of the author (M48).** K13 gave two conflict items for entries 16 and 2, which have the origin `chapter` and `conversation`. The Host refused the call with `conflict_entry_unavailable`, and the next call gave a correct observation. The sentence: "When the input is against an entry of the author, give a conflict item with the id of the entry."
- **F6. Operations that no observation supports (M52).** In P13 the input had only the conflict, and the model gave a merge of entries 2 and 32 and updates of entries 16 and 27, each with an empty list of observations. The sentence: "Update an entry with what the observations add." In P6 (M18) a merge of a foreshadowing entry and an `other` entry was refused with `merge_type_mismatch`. The sentence: "Merge two or more entries of one type into the first of them."

## Statements

Each statement set the due time of one row to the present and changed 1 row. `due.sql` on the tag has the full text of each one. The form is `UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '<project>'::uuid AND work_kind = '<kind>' AND source_id = '<source>'::uuid;`.

| Kind | Count | Times (UTC) |
| --- | --- | --- |
| C (Conversation A) | 2 | 20:27:42, 21:17:15 |
| A (Chapter) | 12 | 20:29:15, 20:34:13, 20:37:19, 20:41:54, 20:46:35, 20:50:10, 20:55:44, 21:01:19, 21:07:31, 21:12:38, 21:17:31 (Chapter 3 changed), 21:21:13 (Chapter 9 changed) |
| B (pass) | 12 | 20:30:38, 20:35:41, 20:38:41, 20:44:27, 20:48:02, 20:51:37, 20:57:53, 21:02:42, 21:10:07, 21:14:46, 21:20:46, 21:23:06 |

## Queries

The script `apps/web/test/support/memory-calibration.ts` has each query in its function `measures`, and each one is read-only. Each query filters on `project_id`.

- Captures: `memory_captures` with the count of `model_attempts` of its Run (RunSteps), its `tool_calls` with the function, the state, the refusal, and the arguments, and the count of `memory_observations` by `kind` and `entry_type`. The time of the end is the latest `agent_run_events.recorded_at` of the Run.
- Passes: `memory_consolidation_passes` with its `observations` (the input items), `outcome`, `operations`, and `bool_and(kind = 'conflict')` over the observations of its input.
- Requests: `model_attempts` joined to `agent_runs` for `run_kind`, the usage, and `decision.kind`.
- Context items: the items of `context_assembly_manifests.payload` with `source_class` `memory_index` or `chapter_list` and their `token_count`.
- Entries after each pass: the latest row of each `memory_entry_id` in `memory_entry_versions`, with its length, origin, state, and flag.
- Runs of the author: `agent_runs` with `run_kind = 'author'`, its RunSteps, and its `tool_calls`.
- Answers: `model_attempts.payload #>> '{decision,text}'` of the last request of each Run of the author (`answers`). The run of `ee880700` did not have this query in the script. The executor read `payload -> 'decision'` of M57, M59, and M62 with a read-only `psql` query on the database of the run, before the database was removed.
- Stops: `checkLimits` counts the requests, sums the reported input, takes the largest input of a capture or a pass and of a Run of the author, and counts the failed captures, the failed passes, and the Runs of the author that are `refused` or `cancelled`.
