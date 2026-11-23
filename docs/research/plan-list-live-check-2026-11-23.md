# Live check of the plan list of a Run (2026-11-23)

This record is the live check of [S9-01: Record the Plan of a Run and Give It to the Run Read](https://github.com/FrankQDWang/StoryOS/issues/435) (item 10) and [ADR 0103](../adr/0103-deliver-a-smaller-stage-9-with-a-plan-list-of-a-run-and-a-subrun-of-one-level-that-the-parent-waits-for.md) section 2. One run on the real Agent Plan route shows, from the ToolCalls and from `getAgentRun`, which Run of the author records a plan, and how the plan changes while the Run works. It is one check of the new Tool and its instruction, not a trial that compares texts.

Raw evidence is on the local tag `evidence/435-plan-list-live-2026-11-23` (commit `f12ae50094013ceff4c67487451058c8165fefe5`). The tag holds the plan, the run script `plan-live.ts`, `run.sh`, `rehearse.sh`, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M14.json`), the Model Attempts, the ToolCalls, the Run rows, the guard log, `loader.log`, `plan-reads.json` (each new plan that `getAgentRun` gave), `measures.json`, and the Server and Worker logs. The tag has no database dump and no Project Export Archive. Before the tag, each of its 28 files (about 790 KB) was examined for a key, an e-mail address, and a home path. None was found; the only secret is the test-only challenge secret of `run.sh`, as on the earlier tags. The coordinator examines the tag again and pushes it.

## Plan

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/435#issuecomment-7094533858)).

Inputs:

- Product code and release package: branch `claude/435-plan-list-8dcb4b` at `8cdd59dfef304c9d47d4d290ca11bf4114f295aa` (tree `8fc2257a2259e1a8427239f1199af97c729c8ff2`), `python3 scripts/package-release.py` on the clean tree (manifest `sha256:5acd55c33c72dd47b91755f65ecff7c0a77365198035ad10d5270b59ae9b3dfd`). Registration `018f0000-0000-7001-8000-00000000fa27`, mapping `storyos.volcengine-agent-plan.responses.create.v13`, Host Tool Registration `018f0000-0000-7001-8000-0000000e0106`. The later commits of the branch before the run change only documents.
- Topology: the topology of #1792, #1818, and #1826. `storyos-server` with `STORYOS_WORKER=0` and one continuous `storyos-worker`, both from the release package and with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, and no Skill directory. The Worker resolves the key by its typed settings (ADR 0050). The run never reads an env file or a key, and it uses no browser.
- Sample: the 10 Chapters of `fixtures/memory-calibration/` (`chapter-01.md` to `chapter-10.md`), in one new Project with one Volume. The script writes each Chapter through `applyAuthorEdit`.
- Script: `plan-live.ts`, started by `rehearse.sh` (the request guard, then `run.sh`). The script writes no product row directly: each row of the schema `storyos` comes from the Server and the Worker. The request guard has its own schema `live_guard` and count triggers that enforce the stop limit and change no product row.
- A rehearsal of all steps on `host_fake` with the same package and a separate database passed: 3 Runs, 6 requests, each `completed`, and each Run read with no plan.

Steps:

1. Write the 10 Chapters in order (at most 8 commands in one minute).
2. Run A, in a new Conversation, with Chapter 3 as the Current Chapter: "请帮我检查第一章到第五章：找出每一处沈知遥的年龄、或者两件事之间相隔的时间前后对不上的地方，把这些地方一一列出来；然后把第三章的第一段改得读起来更顺一些。"
3. Run B, in a new Conversation, with Chapter 1 as the Current Chapter: "第一章的第一句话是什么？"
4. Run C, in a new Conversation, with Chapter 2 as the Current Chapter: "请用三句话概括第二章。"
5. Stop the Worker, then write the measures and the raw evidence.

While each Run works, the script reads `getAgentRun` each 3 seconds and keeps each new `plan`. The quiet time of a Conversation capture is 30 minutes and of a Chapter capture 1 hour, and the script stops the Worker after Run C. Thus no request of the Memory work was planned.

| Item | Expected | Stop at | Actual |
| --- | --- | --- | --- |
| AgentRuns | 3 of the author, 0 of the Memory work | 3, and 1 of another kind | 3 of the author |
| Model requests of Run A | 5 to 15 | 30 | 10 |
| Model requests of Run B | 1 or 2 | 4 | 2 |
| Model requests of Run C | 1 to 3 | 6 | 2 |
| Model requests in all, with the Memory work (0) | 7 to 20 | 40 | 14 |
| Reported input in all | 200,000 to 800,000 tokens | 2,000,000 | 382,005 |
| Reported input of one request | 10,000 to 80,000 tokens | 150,000 | 11,474 to 48,266 |
| Exa and embedding requests | 0 | 1 | 0 |
| Time of Run A | — | 40 minutes | 11 minutes 38 seconds |

No stop occurred, and no Run repeated. Each Run is `completed`, pinned the Registration `fa27`, and has the mapping `v13` on each Model Attempt.

## Result

- **Run A records a plan.** Its first RunStep calls `update_plan` with three steps in Chinese, the first `in_progress`. Before `finish`, it calls `update_plan` again with each step `completed`. Both calls settle with "The plan is recorded." The Run read gives the first plan from 10 seconds after the start, and the final plan after the end.
- **Run B and Run C record no plan.** Each one calls only `finish`, and its Run read has no field `plan`.
- **Finding F1: Run A does not keep the plan current between the two calls.** From RunStep 1 to RunStep 6 (about 11 minutes), the plan shows step 1 `in_progress` and steps 2 and 3 `pending`. In that time the Run reads the Chapters (step 1), searches the manuscript (step 2), and opens a Proposal for Chapter 3 (step 3). The instruction asks for "one step is in_progress while you do it, and a done step is completed". The model recorded the plan before the first step and the final state before `finish`, as the instruction asks, but not the change from one step to the next. As the ticket says, this record reports it and changes no text: a second live run needs the answer of the author.
- Each request of a Run of the author declares `update_plan` (12 Tools) and has the paragraph of the plan one time, also the closing request with `tool_choice` `none` (M10, M12, M14).

## Runs

Time is UTC on 2026-11-23, from 07:48:43 (the Project) to 08:05:55 (the measures). The 10 Chapters took 49 commands until 07:53:46. Input and output are the tokens that the Provider reported.

| Request | Run | RunStep | `previous_response_id` | Input | Output | Decision |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | A | 0 | no | 12,154 | 273 | `update_plan` (step 1 `in_progress`) |
| M2 | A | 1 | yes | 12,493 | 224 | `read_chapter` of Chapters 1, 2, 4, and 5 |
| M3 | A | 2 | yes | 23,624 | 8,482 | `search_manuscript` "岁" |
| M4 | A | 3 | yes | 33,135 | 314 | `search_manuscript`, 4 queries |
| M5 | A | 4 | yes | 34,768 | 391 | `search_manuscript`, 4 queries |
| M6 | A | 5 | yes | 36,461 | 8,401 | `search_manuscript`, 4 queries |
| M7 | A | 6 | yes | 46,999 | 448 | `propose_edit` of the first paragraph of Chapter 3 |
| M8 | A | 7 | yes | 37,553 | 10,583 | `update_plan` (each step `completed`) |
| M9 | A | 8 | yes | 48,186 | 33 | `finish` |
| M10 | A | 9 | yes | 48,266 | 1,159 | answer |
| M11 | B | 0 | no | 12,385 | 80 | `finish` |
| M12 | B | 1 | yes | 12,535 | 28 | answer |
| M13 | C | 0 | no | 11,474 | 430 | `finish` |
| M14 | C | 1 | yes | 11,972 | 63 | answer |

Each of the 21 ToolCalls of Run A is `settled`. The two plans of Run A, from `tool-calls.json`:

| RunStep | Step 1 "通读第一至第五章，摘录年龄与时间线索" | Step 2 "核对沈知遥年龄及事件间隔，整理矛盾清单" | Step 3 "润色第三章第一段并提交修改建议" |
| --- | --- | --- | --- |
| 0 | `in_progress` | `pending` | `pending` |
| 7 | `completed` | `completed` | `completed` |

The reads of `getAgentRun` (`plan-reads.json`):

| Time | Run | Status | Plan |
| --- | --- | --- | --- |
| 07:53:46 | A | `claimed` | none |
| 07:53:56 | A | `claimed` | the plan of RunStep 0 |
| 08:04:56 | A | `claimed` | the plan of RunStep 7 |
| 08:05:24 | A | `completed` | the plan of RunStep 7 |
| 08:05:25 and 08:05:34 | B | `claimed`, `completed` | none |
| 08:05:35 and 08:05:54 | C | `claimed`, `completed` | none |

The answers:

- **Run A** lists four places that do not agree in Chapters 1 to 5 (the years, the count, and the sum of the money in Chapter 3; the direction of the pages of the account book in Chapter 3; the repair time of the cane chair in Chapters 4 and 5; the order of the stop of the rain in Chapter 1), three places that it is not sure about, and the time line that agrees. It names the Proposal for the first paragraph of Chapter 3. This record does not judge the places.
- **Run B** gives the first sentence of Chapter 1: "沈知遥下高铁的时候，雨正下得紧。"
- **Run C** gives one sentence in its closing request (M14), and that sentence refers to "以上三句话" (the three sentences above). The Host keeps no text beside the `finish` call of M13, so no record of this run has the three sentences. #435 does not change how the Host keeps the text of a response. Observation O1: the author sees only the sentence that refers to the summary.
