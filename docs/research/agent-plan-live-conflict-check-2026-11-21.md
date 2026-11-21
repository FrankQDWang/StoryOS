# Agent Plan live run of the SkillPackage conflict-check (2026-11-21)

This record is the live evidence of [S10-03: Ship the SkillPackages of the StoryOS Distribution, with the Conflict Check as the First Package](https://github.com/FrankQDWang/StoryOS/issues/1763) for `S10-EVD-002` and [ADR 0100](../adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md) sections 2 and 3. It shows the effect of the instructions of the package `conflict-check` on the real Agent Plan route. The run is no trial that compares models or prompts, and it does not assert that the model obeys the text.

Raw evidence is on the tag `evidence/1763-live-conflict-check-2026-11-21`. It holds the plan, the two Chapters, the list of the planted conflicts, the run script, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M10.json`), the Model Attempts, the ToolCalls, the guard log, `measures.json` (each answer in full), `entries.json`, `checks.json`, `loader.log`, and the Server and Worker logs. The tag has no database dump and no Project Export Archive.

## Findings

| ID | Finding | Effect |
| --- | --- | --- |
| F1 | **The Run with the selected Skill found the four planted conflicts.** Each item gave the words of Chapter 2, the words of the source with its Chapter and paragraph or its Memory Entry, and one sentence. It reported one more item as a small time difference that was not planted. | The method and the report form of the instructions show in the answer. |
| F2 | **The Run with no selected Skill loaded `conflict-check` from the catalog** with a reason, and it also found the four planted conflicts. It reported two more items as small risks that were not planted. | The Chinese description is sufficient for the model to choose the Skill from the catalog for a plain request. |
| F3 | **No Run of the check made a Proposal or a Memory write.** Each check Run called only `read_chapter` for Chapter 1, `read_memory_entry` for entry 1, and `finish` (and `load_skill` in Run 3). | The rule "no change" shows. |
| F4 | **Each report ended with a statement of what it compared**: the Chapters and the Memory Entry that it read, and what it did not read. Run 2 also said why it made no search: only Chapter 1 is earlier, and it read all of it. | The statement of the instructions shows. |
| F5 | **Neither report decided which side is correct.** Each report asked the author to decide, and Run 3 offered a revision plan on a later request of the author. | No effect: the offer is no change. |
| F6 | **Each report also listed the facts that agree** (for example 老周 sees with the right eye only, the hands stop at 四点十分). The instructions do not ask for this list. | The report is longer. The list does not hide a conflict. Observation. |
| F7 | **The two not-planted items are true readings of the fixture.** 九月初三 to 十月初九 is more than one month, but Chapter 2 says "第一个月". The key is sewn into the 棉袄 that 林舟 takes off before he swims. Each report marked them as small and separate from the conflicts. | No false conflict. Observation of the fixture. |
| F8 | **The requests declared no `hybrid_search_manuscript`,** because the run has no embedding destination. They declared `web_search` and `web_fetch` with no Exa key. The model called none of them. | The instruction "when the request does not give one of them, do the work with the others" had no case to fail. |
| F9 | **In RunStep 1 of Run 2 the model wrote the full report before `finish`** (3,245 output tokens), and the closing request wrote it again (868 output tokens). | The same observation as F3 of the record of 2026-11-15. Output tokens of about one report more. |
| F10 | **In Run 1, the request to remember, the model also told the author that Chapter 2 conflicts with the new entry.** It made no Proposal. | No effect on the check. The model read the Working text of the request. |
| F11 | **The run used the mapping `v9` and Registration `fa23`** of the integration tip. The default writing rules of #1761 were not in the Host instructions. | A later change of the Host instructions can change the form of the report. |

## Setup

| Item | Value |
| --- | --- |
| Product code | `1763-distribution-skillpackages` at `037239c31e2104d069fbcb21fb5057fdff6bca92` (tree `c93b448f000ab71eb39d6e197348b15798bb393c`), on `stage10-author-voice` `efc88f58`. |
| Release package | `python3 scripts/package-release.py` on that clean commit, manifest `sha256:5577a5530fa4c1ee8774e5143e759bb5d2b313036de722539954d6faf58b19e0`. `skill-packages/conflict-check/SKILL.md` of the release package has the bytes of the tracked file (SHA-256 `6b163fdf93e1533623ee8964cf9627e8baf0566c649ea2fd51b9d743229a663b`). |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_SKILLS_DIR` set to `target/release-package/skill-packages`, on `127.0.0.1:8812`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package. Each process environment had only the named variables, `PATH`, and `HOME`. It had no embedding destination and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh with` database. The only direct write was the guard schema `live_guard`. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa23`, mapping `storyos.volcengine-agent-plan.responses.create.v9`. |
| Script | `conflict-check-live.ts`, started by `run.sh` and `rehearse.sh`. It is the run script of #1700 with the steps of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL. |
| Skill | `conflict-check`, package digest `sha256:86deb6fb8d1a4caee81dc88e594398dd47f2fe6c157f7cc8f9cc9f4b92a20aed`. `listSkills` gave it with the description of its `SKILL.md`. |

## Plan and final count

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1763#issuecomment-7059237135)). A rehearsal of all steps on `host_fake` on the same head came first: 3 author Runs, 6 fake requests, and no other Run.

| Item | Planned | Actual |
| --- | --- | --- |
| AgentRuns | 3 (limit 5) | 3, each `completed`, each in a new Conversation |
| Model requests | 12, hard cap 24 | 10 |
| Reported input | about 75,000 tokens | 61,348 tokens |
| Largest request | at most 40,000 | 9,643 |
| Other Runs (Memory work) | 0 | 0 |
| Exa and embedding requests | 0 | 0 |

## Fixed inputs

A new Project 渡口 with the Volume 第一卷 and two Chapters, written through the public commands: Chapter 1 渡口 and Chapter 2 来信, each with 5 paragraphs. Chapter 2 was the current Chapter, so it was the Working text of each Run. The texts are in `chapter-01.md` and `chapter-02.md` of the tag.

| ID | Kind | Chapter 2 | Source |
| --- | --- | --- | --- |
| P1 | Kinship | 妹妹林雪走后的第一个月 | Chapter 1: 姐姐林雪比他大六岁 |
| P2 | Date | 他想起五年前的夏天，父亲出事以后 | Chapter 1: 三年前的冬天，父亲林德在河上翻了船 |
| P3 | Object state | 玻璃光洁如新，一道裂纹也没有 | Chapter 1: 怀表的玻璃裂了一道斜纹 |
| P4 | Memory | 跳进河里，一口气游到了对岸 | Memory Entry 1 of Run 1: 林舟怕水，从小不会游泳 |

Some facts agree on purpose: 老周 sees with the right eye only, the hands of the watch stop at 四点十分, and 林雪 went to the 省城 on the other bank.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. Each Model Attempt is `settled`. `prev`: the request continued the response of the row before it with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 07:27:16 | 1, 0 | `resp_02179524603713758818742e03404164f2bdc6cf7ab386a259f24` | none | required | `write_memory` | 4,557 / 739 |
| M2 | 07:27:31 | 1, 1 | `resp_0217952460525116cb63cadb26a6eb1a5dd8bb71cce3d7f584425` | M1 | required | `finish` | 5,377 / 40 |
| M3 | 07:27:35 | 1, 2 | `resp_021795246056388e0d15f89dc5eb44ad1ad600579228b79d999ea` | M2 | none | final answer | 5,467 / 278 |
| M4 | 07:27:47 | 2, 0 | `resp_021795246067818295492bf4fae3051b4480808ec74710f3a9073` | none | required | `read_chapter`, `read_memory_entry` | 5,337 / 419 |
| M5 | 07:27:55 | 2, 1 | `resp_02179524607658191eb505e0b3b4db7bd42120475941dff56be8d` | M4 | required | `finish` | 6,350 / 3,245 |
| M6 | 07:29:03 | 2, 2 | `resp_0217952461443378b180e7ab5368500e338034e77cb755dde4511` | M5 | none | final answer | 9,643 / 868 |
| M7 | 07:29:23 | 3, 0 | `resp_021795246164573388e10ad0dcd28daa4414437ebb5a8575b7f28` | none | required | `load_skill` | 4,865 / 111 |
| M8 | 07:29:28 | 3, 1 | `resp_0217952461692044c9833b195a1cac5bbbd18e9e4e5ff837cd376` | M7 | required | `read_chapter`, `read_memory_entry` | 5,491 / 111 |
| M9 | 07:29:32 | 3, 2 | `resp_02179524617303572ea91cebe75e8e291705b4c0b59095ea8ff93` | M8 | required | `finish` | 6,179 / 1,856 |
| M10 | 07:30:08 | 3, 3 | `resp_0217952462091716216202cdc086c00ba26009437f217e9f0f925` | M9 | none | final answer | 8,082 / 815 |

Total: **10 model requests, 61,348 reported input tokens, and 8,482 output tokens.** No Retrieve, no recovery, and no refused call occurred.

Each request declared the same 13 functions: `web_search`, `web_fetch`, `propose_edit`, `submit_research`, `read_tool_result`, `load_skill`, `read_skill_resource`, `read_chapter`, `search_manuscript`, `write_memory`, `read_memory_entry`, `search_memory`, and `finish`. Each request had the section `Skill catalog` with the one line of `conflict-check`.

## Run 1: the Memory Entry

Author request, with no selection: "请帮我记住一件事：林舟怕水，从小不会游泳，这是后面一个情节的关键。先别改正文。"

The model called `write_memory` one time: `create`, type `character`, title 林舟, text "林舟怕水，从小不会游泳。作者指明这是后面一个情节的关键，需作为伏笔保留。作者要求暂不修改正文。" Entry 1 has the origin `author_request`. Run 1 made no Proposal (finding F10).

## Run 2: the selected Skill

Author request, with the selection of `conflict-check` in `skill_selections`: "请检查这一章。"

The user message of M4 had the section `Skill instruction` with the exact bytes of `skill-packages/conflict-check/SKILL.md` (byte comparison: equal).

**Each function call:**

| RunStep | Function | Arguments | Result |
| --- | --- | --- | --- |
| 0 | `read_chapter` | `{"chapter": 1, "first_block": null, "last_block": null}` | settled |
| 0 | `read_memory_entry` | `{"entry": 1}` | settled |
| 1 | `finish` | `{}` | settled |

**Each reported item against the planted list:**

| Item of the report | Planted | Result |
| --- | --- | --- |
| 一、人物关系：林雪是"妹妹"还是"姐姐" (第一章第 3 段) | P1 | found |
| 二、父亲出事的时间：五年前的夏天 vs 三年前的冬天 (第一章第 2 段) | P2 | found |
| 三、怀表玻璃：无裂纹 vs 有一道斜纹 (第一章第 2 段) | P3 | found |
| 四、林舟会不会游泳 (记忆条目 1《林舟》) | P4 | found |
| 五、轻微时间出入：第一个月 vs 九月初三到十月初九 | not planted | reported as small (F7) |

Missed: none. The answer starts with "共发现 4 处确定矛盾，另有 1 处轻微的时间出入". Each item has the words of Chapter 2, the words of the source with its place, and one sentence that says what does not agree.

**The statement of what it compared**, from the section 核对范围 of the answer:

> - 已通读：第一章《渡口》全部 5 个段落；记忆条目 1《林舟》全文。
> - 搜索情况：此前章节仅有第一章，且相关事实在第一章内均可直接查到，故未再使用全文检索；记忆索引中仅此一条，已读取。
> - 未涉及：第二章之后的章节尚不存在，无可对照内容。

The last paragraph of the answer starts with "本次仅作核对，未对正文作任何修改。" Run 2 made **no Proposal** and **no Memory write**: no `propose_edit` and no `write_memory` call, no row in `storyos.proposals` for the Run, and entry 1 stayed at version 1.

## Run 3: no selected Skill, a plain request

Author request, in a new Conversation with no selection: "请帮我看看这一章和前面的章节、和你记住的事情有没有前后矛盾的地方。"

**The model called `load_skill` for `conflict-check`** in RunStep 0, with the reason "作者要求核对当前章节与前面章节、记忆之间的前后矛盾，正是该技能的用途。" M8 sent the output of the call: the exact bytes of the `SKILL.md` (byte comparison: equal).

| RunStep | Function | Arguments | Result |
| --- | --- | --- | --- |
| 0 | `load_skill` | `{"skill": "conflict-check", "reason": "…"}` | settled |
| 1 | `read_chapter` | `{"chapter": 1, "first_block": null, "last_block": null}` | settled |
| 1 | `read_memory_entry` | `{"entry": 1}` | settled |
| 2 | `finish` | `{}` | settled |

The answer reported P1, P2, P3, and P4 as 4 处明确矛盾, and two not-planted items as 轻微隐患: the key in the 棉袄, and 第一个月 against 十月初九 (F7). Its section 核对范围 named Chapter 1 (5 paragraphs) and Memory Entry 1 as read, and said that the book has no other Chapter. Run 3 made no Proposal and no Memory write.

## Key check

The Server log (93 lines), the Worker log (13 lines), the ten wire payloads, and the other files of the tag have no `Authorization` header, no bearer token, no `x-api-key`, no e-mail address, and no home path.
