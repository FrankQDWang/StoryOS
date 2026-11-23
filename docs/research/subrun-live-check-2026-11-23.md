# Live check of the reads of a Subrun (2026-11-23)

This record is the live check of [S9-02B: Give the Subruns of a Run and the Record of One Subrun to the Reads](https://github.com/FrankQDWang/StoryOS/issues/1836) (item 4) and [ADR 0103](../adr/0103-deliver-a-smaller-stage-9-with-a-plan-list-of-a-run-and-a-subrun-of-one-level-that-the-parent-waits-for.md) section 3A. One run on the real Agent Plan route shows which Run of the author starts a Subrun, what `getAgentRun` and `getSubrun` give while the Subrun works and after its end, and what the Subrun does near its limit of 12 RunSteps. It is one check, not a trial that compares texts.

Raw evidence is on the local tag `evidence/436-subrun-live-2026-11-23`, in `docs/research/subrun-live-check-2026-11-23/evidence/`. The tag holds the plan, the run script `subrun-live.ts`, `run.sh`, `rehearse.sh`, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M25.json`), the Model Attempts, the ToolCalls, the AgentRun rows, the Subrun Requests, the guard log, `loader.log`, `subruns-reads.json` (each new `subruns` of `getAgentRun`), `record-reads.json` (each new `getSubrun` record while the Subrun worked), `measures.json`, and the Server and Worker logs. The tag has no database dump and no Project Export Archive. Before the tag, each file was examined for a key, an e-mail address, and a home path. None was found; the only secret is the test-only challenge secret of `run.sh`, as on the earlier tags. The coordinator examines the tag again and pushes it.

## Plan

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1836#issuecomment-7106944226)).

- Product code and release package: branch `claude/1836-subrun-reads` at `446ba984fc45075bdfb9e73b468f45e6c9a5f6b1` (tree `76f93b7effeb52fe31b6b5e6ed4bf5a6e81c9f0b`), `python3 scripts/package-release.py` on the clean tree (manifest `sha256:8776b01a71d71fa453b3dae66783a24d31960647582b3acc57417fcda771771a`). Registration `018f0000-0000-7001-8000-00000000fa30`, mapping `storyos.volcengine-agent-plan.responses.create.v16`, Host Tool Registration `018f0000-0000-7001-8000-0000000e0107`.
- Topology: the topology of the live check of S9-01 (#435). `storyos-server` with `STORYOS_WORKER=0` and one continuous `storyos-worker`, both from the release package and with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, and no Skill directory. The run never reads an env file or a key, and it uses no browser.
- Sample: the 10 Chapters of `fixtures/memory-calibration/`, in one new Project with one Volume, written through `applyAuthorEdit`. Chapter 1 is the Current Chapter of each Run. Each Run is in a new Conversation.
- Script: `subrun-live.ts`, started by `rehearse.sh` (the request guard, then `run.sh`). The script writes no product row directly. The request guard of the schema `live_guard` counts model requests and AgentRuns with triggers and changes no product row.
- Two rehearsals on `host_fake` passed. In the second, Run C sent the Subrun script of the fake model (`SCRIPT:start_subrun`, only when `STORYOS_REHEARSAL_SUBRUN=1`, which the live run does not set): the reads gave the Subrun `working`, then `completed`, and its record.

The three author messages:

- Run A: "沈知遥在第一章到第十章里每次提到自己的家人时是怎么说的？请把出处列出来，并指出前后不一致的地方。"
- Run B: "第一章的第一句话是什么？"
- Run C: "请交给一个子任务：从第一章到第十章逐章通读，每一步只读一章，然后列出每一章里出现的地名。"

While each Run worked, the script read `getAgentRun` each 3 seconds and kept each new `subruns`. For a Subrun with the state `working`, it also read `getSubrun`. The script stopped the Worker after Run C, so no Memory work started.

| Item | Expected | Stop at | Actual |
| --- | --- | --- | --- |
| AgentRuns of the author | 3 | 3 | 3, each `completed` |
| AgentRuns of the Memory work | 0 | 1 | 0 |
| Subruns of one Run | Run A 0 or 1, Run B 0, Run C 1 | more than 4 | Run A 0, Run B 0, Run C 1 |
| RunSteps of one Subrun | Run C 10 to 12 | more than 12 | 11 |
| Model requests of the parent Run A | 3 to 15 | 30 | 9 |
| Model requests of the parent Run B | 1 or 2 | 4 | 2 |
| Model requests of the parent Run C | 2 to 4 | 8 | 3 |
| Model requests in all | 20 to 50 | 80 | 25 (with 11 of the Subrun) |
| Reported input in all | 300,000 to 1,500,000 tokens | 4,000,000 | 621,011 |
| Reported input of one request | 5,000 to 80,000 tokens | 150,000 | 2,095 to 75,583 |
| Exa and embedding requests | 0 | 1 | 0 |
| Time of Run A, Run B, and Run C | — | 40, 10, and 40 minutes | 10 min 6 s, 10 s, 6 min 20 s |

No stop occurred, and no Run repeated. Each AgentRun, also the Subrun, pinned the Registration `fa30` and has the mapping `v16` on each Model Attempt.

## Result

- **Run A reads by itself.** It starts no Subrun. It searches the manuscript, reads Chapters 2 to 10 in two RunSteps, searches again, and answers with a table for each Chapter of the places where 沈知遥 talks about her family, and with eight points that do not agree or need attention. This record does not judge the points.
- **Run B starts no Subrun.** It calls `finish` and gives the first sentence of Chapter 1: "沈知遥下高铁的时候，雨正下得紧。"
- **Run C starts one Subrun, and the Subrun ends `completed` before its last RunStep.** The Subrun reads one Chapter in each of RunSteps 0 to 7. RunSteps 8 and 9 have a sentence near the limit. The Subrun gives its final answer at RunStep 10, and RunStep 11, the last, does not occur. The parent calls `finish` and gives the list of the Subrun to the author.
- **The reads agree with the stored records.** `getAgentRun` of Run A and Run B never has the field `subruns`. For Run C, `subruns` has one entry from 19 seconds after the start: `working` while the Subrun worked, then `completed`. `getSubrun` counted the RunSteps from 0 to 10 while the Subrun worked and 11 after its end. No read gave a line of the Chapter text of the sample.
- **Finding F1: the requests of the Subrun replay the earlier calls as one block.** See [Finding F1](#finding-f1).
- Observation O1: at RunStep 8, after the sentence "after this step, 3 steps are left", the Subrun reads Chapters 9 and 10 in one response, against the brief "每次只调用一次 read_chapter". With one Chapter in each RunStep, the task also fits: Chapter 9 at 8, Chapter 10 at 9, `finish` at 10, and the answer at 11. The answer of the parent (M25) still says "每次只读一章".

## Runs

Time is UTC on 2026-11-23, from 16:19:13 (the Project) to 16:40:55 (the measures). The Project and the 10 Chapters took 44 commands until 16:24:16. Input and output are the tokens that the Provider reported. C/S1 is the Subrun of Run C.

| Request | Run | RunStep | Input | Output | Decision |
| --- | --- | --- | --- | --- | --- |
| M1 | A | 0 | 12,762 | 415 | `update_plan` |
| M2 | A | 1 | 13,243 | 132 | `search_manuscript` "爷爷", "爸", "妈", "家", "奶奶" |
| M3 | A | 2 | 24,103 | 352 | `update_plan` |
| M4 | A | 3 | 24,502 | 219 | `read_chapter` of Chapters 2, 3, 4, 5 |
| M5 | A | 4 | 35,487 | 272 | `read_chapter` of Chapters 6, 7, 8, 9, 10 |
| M6 | A | 5 | 49,440 | 6,571 | `search_manuscript` "父亲", "知南", "弟弟", "沈怀山", "明川" |
| M7 | A | 6 | 62,878 | 12,584 | `update_plan` |
| M8 | A | 7 | 75,509 | 24 | `finish` |
| M9 | A | 8 | 75,583 | 5,163 | answer |
| M10 | B | 0 | 12,737 | 142 | `finish` |
| M11 | B | 1 | 12,951 | 56 | answer |
| M12 | C | 0 | 12,759 | 750 | `start_subrun` |
| M13 | C/S1 | 0 | 2,095 | 73 | `read_chapter` of Chapter 1 |
| M14 | C/S1 | 1 | 5,159 | 142 | `read_chapter` of Chapter 2 |
| M15 | C/S1 | 2 | 7,862 | 689 | `read_chapter` of Chapter 3 |
| M16 | C/S1 | 3 | 10,732 | 1,047 | `read_chapter` of Chapter 4 |
| M17 | C/S1 | 4 | 13,332 | 1,203 | text, `read_chapter` of Chapter 5 |
| M18 | C/S1 | 5 | 16,120 | 1,617 | `read_chapter` of Chapter 6 |
| M19 | C/S1 | 6 | 18,733 | 968 | `read_chapter` of Chapter 7 |
| M20 | C/S1 | 7 | 21,536 | 1,483 | text, `read_chapter` of Chapter 8 |
| M21 | C/S1 | 8 | 24,222 | 1,661 | `read_chapter` of Chapters 9, 10 |
| M22 | C/S1 | 9 | 30,066 | 3,868 | `finish` |
| M23 | C/S1 | 10 | 30,089 | 4,364 | answer |
| M24 | C | 1 | 14,511 | 41 | `finish` |
| M25 | C | 2 | 14,600 | 853 | answer |

Each ToolCall is `settled`. The requests of the parents have `previous_response_id` after RunStep 0; the requests of the Subrun have none. Each request of the Subrun has the sentence "You have at most 12 steps for this task." of its instructions.

### The `start_subrun` call

M12 calls `start_subrun` with the title "通读一至十章，列出地名" and this brief, exactly:

```text
任务：通读小说第一章到第十章（共10章），逐章列出其中出现的地名。

阅读要求（必须严格遵守）：
1. 按章节顺序读：先读第一章，再读第二章……直到第十章。
2. 每一步只读一章：每次只调用一次 read_chapter（chapter 参数依次为 1 到 10，first_block 和 last_block 都传 null，读完整章），读完并记录该章地名后，再读下一章。不要一次调用多个 read_chapter，也不要跳章。
3. 第一章是当前工作章节，也要用 read_chapter(chapter=1) 正式读一遍。

什么算"地名"：
- 真实或虚构的城市、乡镇、车站、河流、街道、桥梁、村庄、区域等地名（如：上海、杭州、青溪站、青溪镇、临水街、青溪等）。
- 具体的店铺、建筑、场所名（如茶馆、宾馆、商场等有专名的地点）也请列出，并标注它是"场所"。
- 不要列入：人名、机构/公司名（除非同时是地名）、交通工具班次号（如 G7351 次列车不算地名，但"青溪站"算）、茶名等。
- 同一地名在同一章重复出现只列一次；若同一地名在多章出现，每章都列出（便于逐章对照）。

输出格式（中文）：按第一章到第十章分节，每章形如：
第一章《归》
- 青溪站（车站）
- 上海（城市）
……
若某章没有出现任何地名，写"（本章无地名）"。
全部列完后，另附一节"跨章重复地名"，把在两章及以上出现的地名及其出现章节号列出。只做客观摘录，不要评价、不要改写文本、不要提出修改建议。
```

### The reads while the Subrun worked

| Time | Read | Value |
| --- | --- | --- |
| 16:34:32 | `getAgentRun` of Run C | `claimed`, no field `subruns` |
| 16:34:52 | `getAgentRun` of Run C | `claimed`, one entry "通读一至十章，列出地名" `working` |
| 16:34:52 to 16:39:14 | `getSubrun` | `working`; `step_count` 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10; `steps` 0 to 12 entries |
| 16:40:37 | `getAgentRun` of Run C | `claimed`, the entry `completed` |
| 16:40:53 | `getAgentRun` of Run C | `completed`, the entry `completed` |

### The record of the Subrun

`getSubrun` after the end gives the title, the brief above, `state` `completed`, `step_count` 11, the answer, and these 12 entries of `steps`:

1. to 4. `chapter_read` of "第一章 归", "第二章 箱", "第三章 账", "第四章 椅".
5. `text`: "我注意到一次调用了多个 read_chapter，不符合"每步只读一章"的要求。后续章节我将严格每次只读一章。现在继续读第五章。"
6. to 8. `chapter_read` of "第五章 开", "第六章 报", "第七章 约".
9. `text`: "我已读完前七章（本步系统执行了多个调用，后续严格按每步一章进行）。现在读第八章。"
10. to 12. `chapter_read` of "第八章 钥", "第九章 桥", "第十章 风".

The answer lists the place names of each Chapter and the names in more than one Chapter (青溪 and 临水茶馆 in each Chapter). It is the result text of the `start_subrun` call, exactly. The `finish` call of M22 gives no entry.

### The RunSteps near the limit

| RunStep | Sentence of the request | What the Subrun did |
| --- | --- | --- |
| 8 (M21) | "Step limit: after this step, 3 steps are left. Complete the necessary work, and call finish in time." | Read Chapters 9 and 10 in one response (O1). |
| 9 (M22) | "Step limit: after this step, 2 steps are left. Complete the necessary work, and call finish in time." | Called `finish`. |
| 10 (M23) | none: the closing request has `tool_choice` `none` | Gave the final answer. |

## Finding F1

The two `text` entries say that one step called several `read_chapter` at one time. The stored ToolCalls do not agree: RunSteps 0 to 7 each have one call. The wire payloads show the cause. A request of the Subrun has no `previous_response_id`, so it replays the earlier RunSteps: the brief, then each earlier `function_call`, then each output (`crates/storyos-adapter-volcengine-responses/src/wire.rs`, `input`). M17 thus sends four calls before four outputs, and no text of an earlier response. In that form, the earlier RunSteps look like one response with several calls. The Subrun wrote that wrong statement two times, and the reads give it to the author as `text` entries. #1836 changes no execution of S9-02, so this record changes nothing. The finding goes to the coordinator.
