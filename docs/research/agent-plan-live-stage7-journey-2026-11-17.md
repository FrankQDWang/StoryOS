# Agent Plan live run of the Stage 7 journey (2026-11-17)

This record is the live evidence of [Recovery, the Stage 7 Facts of the Recovery Drill, and the Recorded Live Run](https://github.com/FrankQDWang/StoryOS/issues/1676) (S7-E1b) for [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md).

One run on the real Agent Plan route did the Memory steps and the manuscript steps of the journey `S7-JRN-001` in one Project:

1. Three Chapters of the original sample `fixtures/memory-calibration/` went into a new Project.
2. In Conversation A, the author asked the Agent to remember a foreshadowing. The model made one entry with `write_memory`.
3. Statements of the test administrator connection made the capture rows due. Four captures and two passes followed.
4. The author changed Chapter 3. One more capture and one more pass followed. The pass flagged the entry of the author, because the change made its text wrong.
5. In Conversation B, the author asked a question that needs the entry and the text of Chapter 2. The Memory Index had the flag, and the answer told the author about it.

No stop of the plan occurred.

Raw evidence is on the tag `evidence/s7-journey-2026-11-17` (commit `98346336`). It holds:

- the plan;
- the wire payload of each request (`wire/M1.json` to `wire/M19.json`);
- the Model Attempts, the ToolCalls, and the Embedding Attempts;
- the request guard and its log;
- the statements (`due.sql`) and the table of each check of the stops (`checks.json`);
- `measures.json`, `entries.json` (the entry set after each step), and `loader.log`;
- the run script, and the Server and Worker logs.

The tag has no database dump and no Project Export Archive.

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/1676-s7-e1b` at `b0a39fdbe983d2dd1a32e101eaf090f2538af237` (tree `71842c82`). That is `stage7-memory` `d87e1781` with the fix of #1685 (PR #1687) and S7-D7, and the store tests and drill facts of this ticket, which are not in the package. |
| Release package | `make release-package` on that clean commit, manifest `sha256:8896f27471b2d65e585519a1e34a260bebd6737e237db5d0740fa8ea31aa48e4`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` on `127.0.0.1:8762`, and one continuous `storyos-worker`. Both came from the release package, and both had `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` and `STORYOS_EMBEDDING_DESTINATION=volcengine_agent_plan`. Each process environment had only the named variables, `PATH`, and `HOME`. It had no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh up --interactive` database. The direct writes were the guard schema `live_guard` and the 5 statements of the section [Statements](#statements). The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`. Embedding Registration `018f0000-0000-7001-8000-0000000eb002`. |
| Script | `node stage7-journey-live.ts <server> <worker pid> <out dir>`, started by `run.sh`. Both files are on the tag. The script is the loader of S7-D7 (`apps/web/test/support/memory-calibration.ts`) with the steps of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL. |
| Sample | `chapter-01.md` to `chapter-03.md` of `fixtures/memory-calibration/` (25,818 characters in the three files), and later `chapter-03-changed.md`. The text is original (`PROVENANCE.md`). |

## Plan and final count

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1676#issuecomment-6963618962)). The coordinator confirmed it ([confirmation](https://github.com/FrankQDWang/StoryOS/issues/1676#issuecomment-6963684931)). A rehearsal of all steps on `host_fake` came first, on the same package and a separate database. It took 7.5 minutes and had no error.

- **Planned:** 25 model requests, at most 45. About 270,000 reported input tokens, at most 600,000. About 40 embedding requests, at most 70. No Exa exchange.
- **Request guard:** the `live_guard` triggers of the [record of S7-D6b](agent-plan-live-memory-conflict-2026-11-17.md), counted at the claim: model 45, Exa 0, embedding 70.

One change to the plan: the plan said that the text of the Chapters goes in "through the editor". The script sent the same public commands that the editor sends (`createChapter`, `setCurrentChapter`, `applyAuthorEdit` with `replace_selection`, `split_block`, and `replace_block_selection`) through the generated client, as the S7-D7 run did. No browser was used.

Final count: **19 model requests, 141,474 reported input tokens, and 34,016 output tokens**, inside each limit. There were 13 embedding requests and no Exa exchange. Each Model Attempt is `settled`, and each Run is `completed`. One ToolCall was refused (finding O2).

| Kind | Requests | Input | Output | Largest input |
| --- | --- | --- | --- | --- |
| Runs of the author (2) | 6 | 83,106 | 3,166 | 19,117 |
| Captures (5) | 10 | 46,805 | 12,748 | 7,231 |
| Passes (3) | 3 | 11,563 | 18,102 | 4,993 |

The run took 13 minutes 40 seconds, from 23:26:41 to 23:40:21 UTC. The Worker stopped right after the answer of step 3. The row of Conversation B then had a due time 30 minutes later, so no capture of Conversation B started.

## The checks of the stops

The guard triggers check only the request counts. The script checked each other stop with read-only SQL in each poll of each wait, and once more before each step and each statement:

- a capture request or a pass request above 60,000 reported input tokens;
- a request of a Run of the author above 150,000;
- a total above 600,000;
- a capture or a pass with the outcome `failed`, or a Run of the author that is `refused` or `cancelled`.

Each check before a step or a statement (UTC, from `checks.json`):

| Time | Before | Requests | Input | Largest capture or pass | Largest author | Failed | Embedding | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 23:27:45 | step 1 | 0 | 0 | none | none | 0 | 0 | pass |
| 23:28:11 | statement C1 | 3 | 34,237 | none | 11,719 | 0 | 3 | pass |
| 23:28:27 | statement A1 | 5 | 39,117 | 2,599 | 11,719 | 0 | 13 | pass |
| 23:33:26 | statement B1 | 10 | 59,849 | 5,274 | 11,719 | 0 | 13 | pass |
| 23:36:45 | statement A2 | 11 | 63,933 | 5,274 | 11,719 | 0 | 13 | pass |
| 23:38:19 | statement B2 | 15 | 87,612 | 7,231 | 11,719 | 0 | 13 | pass |
| 23:39:28 | step 3 | 16 | 92,605 | 7,231 | 11,719 | 0 | 13 | pass |
| 23:40:20 | the end | 19 | 141,474 | 7,231 | 19,117 | 0 | 13 | pass |

## Statements

Each statement is one `UPDATE storyos.memory_work_schedules SET due_at = now()` for the Project, with the kind and the source. Each one changed its planned row count. The text of each statement is in `due.sql`.

| Statement | Time | Rows | Reason |
| --- | --- | --- | --- |
| C1, Conversation A | 23:28:11 | 1 | The quiet time of a Conversation is 30 minutes. |
| A1, Chapters 1 to 3 | 23:28:27 | 3 | The quiet time of a Chapter is 1 hour. No row was `held`: each commit came about 1 minute before the Run of step 1 completed. The Worker started the 3 captures at the Chapter rate, one each 2 minutes, with no other statement. |
| B1, the pass | 23:33:26 | 1 | After pass P1 settled. The due time of the row was 30 minutes after the start of P1. |
| A2, the changed Chapter 3 | 23:36:45 | 1 | The next due time of the Chapter was 6 hours after its last capture. |
| B2, the pass | 23:38:20 | 1 | After the capture of the changed Chapter 3 settled. |

Pass P1 started with no statement, at the settlement of the first capture with observations (Chapter 1).

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. Each later request of a Run of the author continued the response before it with `previous_response_id`. A capture or a pass request has no `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | 23:27:45 | step 1, 0 | `resp_0217949580656886525c18cf4d9efd868f038535426f8169889f5` | required | `write_memory` | 10,890 / 653 |
| M2 | 23:27:58 | step 1, 1 | `resp_021794958078866746abb8ea69ccb3d7a75bd7d82f8063fbc34b5` | required | `finish` | 11,628 / 40 |
| M3 | 23:28:02 | step 1, 2 | `resp_0217949580829364e0d847e114bd906533eadfa024bfe6d142070` | none | final answer | 11,719 / 194 |
| M4 | 23:28:11 | K1 Conversation A, 0 | `resp_0217949580919480736cb0c996dcf6699b9adc2424c5dbcdc6bbb` | required | `read_memory_entry` | 2,281 / 202 |
| M5 | 23:28:18 | K1, 1 | `resp_021794958098944f9405f279a0f30d2c73a697bf053f75a18384c` | required | `record_memory_observations` (none) | 2,599 / 170 |
| M6 | 23:28:27 | K2 Chapter 1, 0 | `resp_0217949581077184e0d847e114bd906533eadfa024bfe6dd90337` | required | `record_memory_observations` (5) | 4,265 / 2,092 |
| M7 | 23:29:15 | P1, 0 | `resp_02179495815580023c5b6bc0dca724893eb7d2b1f969c7c4f3c94` | required | `apply_memory_operations` (7) | 2,486 / 3,698 |
| M8 | 23:30:27 | K3 Chapter 2, 0 | `resp_02179495822806920c19fad7de95dc34d140d8afb6e0840ab70fc` | required | `read_memory_entry` ×4 | 4,306 / 1,263 |
| M9 | 23:31:01 | K3, 1 | `resp_021794958261410c2eb6daa98a94139fa98701b99e0cb3ccb6fc4` | required | `record_memory_observations` (4) | 5,274 / 2,591 |
| M10 | 23:32:27 | K4 Chapter 3, 0 | `resp_021794958348090746abb8ea69ccb3d7a75bd7d82f8063fac08b5` | required | `record_memory_observations` (4) | 4,401 / 2,462 |
| M11 | 23:33:26 | P2, 0 | `resp_021794958407010bbc1b38287ece59059a49659ad58bad92481d4` | required | `apply_memory_operations` (10) | 4,084 / 10,802 |
| M12 | 23:36:45 | K5 changed Chapter 3, 0 | `resp_02179495860586916a297e698c513f628e682a266b1360994c8b2` | required | `read_memory_entry` ×3 | 4,774 / 542 |
| M13 | 23:37:00 | K5, 1 | `resp_021794958621164b869c66ddd8a6a297966635e3152d68281de04` | required | `record_memory_observations`, refused | 5,657 / 1,595 |
| M14 | 23:37:35 | K5, 2 | `resp_021794958656142e41c82dd1debd95d24831350e3c3a877eb8b97` | required | `search_manuscript` ×4 | 6,017 / 1,302 |
| M15 | 23:38:06 | K5, 3 | `resp_0217949586864636cb63cadb26a6eb1a5dd8bb71cce3d7f6a2ca5` | required | `record_memory_observations` (2) | 7,231 / 529 |
| M16 | 23:38:20 | P3, 0 | `resp_021794958700261f0644a9747ad3d38d721f78a849566c6179abe` | required | `apply_memory_operations` (5) | 4,993 / 3,602 |
| M17 | 23:39:28 | step 3, 0 | `resp_0217949587685844e0d847e114bd906533eadfa024bfe6d682c0f` | required | `read_chapter` ×2 | 11,917 / 274 |
| M18 | 23:39:35 | step 3, 1 | `resp_021794958775597bbc1b38287ece59059a49659ad58bad9773094` | required | `finish` | 17,835 / 1,232 |
| M19 | 23:40:02 | step 3, 2 | `resp_0217949588026529714750b4c92df19b7d86c8470c87ebfa85a35` | none | final answer | 19,117 / 773 |

M1 to M3 and M17 to M19 declared the twelve Tools of a Run of the author, `hybrid_search_manuscript` included. Each capture request declared the four strict Tools of a capture. Each pass request declared `apply_memory_operations`.

## Step 1: the entry of the author

Run `01a1ebd6-7f00-76d3-8c92-faa72e9f65d3` in Conversation A, with the message "请帮我记住一个伏笔：柜台底下那只上了锁的樟木箱，钥匙其实一直在赵德明手里。后面第八章才会打开它，里面的信和收据会证明顾青禾出了一半房钱。正文现在还没写到，先别改正文。"

The model called `write_memory` one time. It made entry 1, `foreshadowing`, origin `author_request`, title 柜台下的樟木箱, with a text of 257 characters. The text has each fact of the message. It also has one detail that the author did not say: the purchase offer of 八十万 from Chapter 3 (finding O1). The answer told the author that the Agent remembered the entry and did not change the text.

## The captures

The time is the claim time (UTC). The time in seconds is from the start of the capture to its last event.

| # | Source | Start | Time (s) | RunSteps | Tool calls | Observations | Outcome |
| --- | --- | --- | --- | --- | --- | --- | --- |
| K1 | Conversation A | 23:28:11 | 12 | 2 | `read_memory_entry`, `record_memory_observations` with an empty list | none | `no_observation` |
| K2 | Chapter 1 | 23:28:27 | 48 | 1 | `record_memory_observations` | character 2, relationship 1, major event 1, foreshadowing 1 | `observations_recorded` |
| K3 | Chapter 2 | 23:30:27 | 102 | 2 | `read_memory_entry` ×4, `record_memory_observations` | character 1, relationship 1, major event 1, foreshadowing 1 | `observations_recorded` |
| K4 | Chapter 3 | 23:32:27 | 58 | 1 | `record_memory_observations` | character 2, relationship 1, foreshadowing 1 | `observations_recorded` |
| K5 | changed Chapter 3 | 23:36:45 | 93 | 4 | `read_memory_entry` ×3, `record_memory_observations` refused, `search_manuscript` ×4, `record_memory_observations` | major event 1, foreshadowing 1 | `observations_recorded` |

- K1 read entry 1 and recorded nothing: the only fact of Conversation A was already an entry.
- K5 read entries 8, 10, and 13. Entries 8 and 10 had the old offer of 八十万. Its first call had two conflict items, for entries 8 and 10, and one observation. The Host refused the call (finding O2). Then the model searched the manuscript for 80万, 八十万, 一百二十万, and 程立, and it recorded the new offer of 一百二十万 as a `major_event` observation, with the earlier observation again.
- Each of the 15 observations is `consumed` after the passes.

## The passes

| Pass | Start | Input observations | Operations | Result |
| --- | --- | --- | --- | --- |
| P1 | 23:29:15 | 5 (K2) | 7 `create` | "The Host applied 7 operations and refused 0, and the pass is complete." |
| P2 | 23:33:26 | 8 (K3, K4) | 4 `update` (entries 3, 4, 5, 8), 6 `create` | applied 10, refused 0 |
| P3 | 23:38:20 | 2 (K5) | `flag_conflict` on entry 1, 4 `update` (entries 4, 8, 10, 13) | applied 5, refused 0 |

After P3 the Project has 14 entries, each `current`. Entry 1 has the origin `author_request`, and each other entry has the origin `chapter`:

- 5 characters (entries 2, 3, 4, 9, 10);
- 4 relationships (5, 6, 11, 12);
- 1 major event (7);
- 4 foreshadowing entries (1, 8, 13, 14).

P3 did not change the text of entry 1. It wrote version 2 of entry 1 with the same text, the cause `conflict_flag`, and the reason "第三章原文收购价为120万，全稿无80万，与作者条目的八十万设定冲突。" P3 changed entries 8 and 10, which have the origin `chapter`, to the new offer.

## Step 3: the question with the flag

Run `01a1ebe1-38de-73b0-a73d-f4164089c7a5` in Conversation B, with the message "知遥第一次看到那只上锁的箱子时是什么情形？钥匙后面会在谁手里？请引用正文里的原句回答，不要修改正文。"

- Before the Run, the index work of Chapters 1 and 2 was complete: 5 and 4 passage vectors. The index of the changed Chapter 3 was not made again, because its quiet time of 10 minutes ended at 23:46:45, after the run.
- The Memory Index of M17 had the line `{"conflict_flag":true,"description":"上锁樟木箱在柜台下，钥匙一直在赵德明手里，第八章才打开。","id":1,"title":"柜台下的樟木箱"}`.
- The model read Chapter 2 and Chapter 1 with `read_chapter`. It did not call `search_manuscript`, `hybrid_search_manuscript`, or `read_memory_entry` (finding O3).
- The answer quoted the paragraphs of Chapter 2 in which 知遥 finds the box. For the key, it said that Chapters 1 to 3 do not tell who has it. It said that the memory of the Project puts the key with 赵德明 in Chapter 8, that Chapter 8 is not written, and that this entry has a conflict flag. The answer did not change the text, and the Project has no Proposal.

## How the model used null after #1685

The fix of #1685 makes the key descriptions say what null means, and the Host reads a missing key that the kind does not use as null.

- **Captures.** The run has 6 `record_memory_observations` calls. The Host refused none of them as `intake_malformed`. Each item sent each key. An item of the kind `observation` had `"entry": null` and `"reason": null`. A conflict item had `"text": null`, `"type": null`, and `"title": null`.
- **Passes.** The 22 operations of P1, P2, and P3 each sent each key:
  - a `create` had `"entry": null`, `"reason": null`, and `"entries": null`;
  - an `update` had `"type": null`, `"title": null`, `"reason": null`, and `"entries": null`, with a new text and a new description, so the type and the title of the entry stayed the same;
  - the `flag_conflict` had `"text": null`, `"type": null`, `"title": null`, and `"description": null`.
- Each of the 22 operations applied, and the Host refused none.
- For comparison, the [S7-D5b record](agent-plan-live-chapter-capture-2026-11-17.md), finding O1, had two `intake_malformed` refusals, because the calls did not have the nullable keys `entry` and `reason`.

## The journey steps that this run does not repeat

- **Step 6 of `S7-JRN-001`** (a conflict of a later Chapter with an entry of the author, then the change of the author). The [S7-D6b record](agent-plan-live-memory-conflict-2026-11-17.md) has the live evidence. This run adds a second flag, on an entry that `write_memory` made (P3 above).
- **Hybrid search.** The model did not choose it in step 3. The [S7-C3c record](agent-plan-live-hybrid-search-2026-11-17.md) has the live evidence of `hybrid_search_manuscript`.

## Key check

The Server log (89 lines), the Worker log (72 lines), the 19 wire payloads, and the exported rows have no `Authorization` header, no bearer token, no `x-api-key`, no home path, no e-mail address, and no database URL. The two logs have no text of the Chapters, of a message, of an observation, or of an entry (ADR 0047). The two script files on the tag name the worktree as `<worktree>`.

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| O1 | The entry that `write_memory` made in step 1 has a detail that the author did not say: the offer of 八十万 from Chapter 3. When the author changed Chapter 3, this detail made the entry of the author wrong. P3 flagged it and did not change it, as ADR 0094 requires for an entry of the author. | The author must change or confirm entry 1. | Observation for the coordinator. |
| O2 | The first call of K5 had conflict items for entries 8 and 10, which have the origin `chapter`. The Host refused the whole call as `conflict_entry_unavailable`. A conflict item can name only an entry of the author. The refusal sentence said this, and the model then sent the facts as observations, which P3 applied. | 2 more capture requests (M14, M15), about 13,200 input tokens. | Observation. |
| O3 | Step 3 used `read_chapter` for Chapters 1 and 2, and no search Tool. So the run has no embedding request for a query. | Hybrid search was not used in this run. | Observation. |
| O4 | P2 had 10,802 output tokens for 10 operations. | Output tokens only. | Observation. |
