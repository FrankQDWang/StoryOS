# Agent Plan live Skill load run (2026-11-15)

This record is the live evidence of [Bound the Loads of a Run, Use Two Skills, and Record a Live Load](https://github.com/FrankQDWang/StoryOS/issues/1531) for [ADR 0089](../adr/0089-deliver-standard-skills-from-the-deployment-and-load-them-with-host-tools.md). It records two Runs on the real Agent Plan route with the two fixture Skills of S6-01a. In Run 1 the model loaded the Skill that fits the author request from the catalog. In Run 2 the route accepted the section `Skill instruction` of a selected Skill, and the answer followed rule 1 of that Skill.

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/s6-03c-load-bounds` at `f09a2755af5be1830bae70a94f90c3dd7a6de47f` (tree `45b914d0`), on `stage6-skills` `e13aebe5`. |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:784fbeda0ec01c15ea6d81ce32161fb5df43d08b1db20ea65a9c0ac7a03e405f`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_SKILLS_DIR` on `fixtures/skill-packages`, on `127.0.0.1:8748`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with a process environment that has only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout, which it found from its working directory in the worktree. This run never read, copied, or printed that file or a key. |
| Database | New `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The only direct writes were the guard schema `live_guard`. The Project, the Chapter, and its text went through the panel. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa21`, mapping `storyos.volcengine-agent-plan.responses.create.v7`, profile `storyos.volcengine-agent-plan.execution.v1`, grant scope `project_writing_assistance_v4`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`, with `localStorage`, `sessionStorage`, and IndexedDB of the origin clear before the first step. New Project 技能加载实测, Volume 第一卷, Chapter 雨夜 with one paragraph typed in the editor: "雨下了一整夜。林舟站在渡口，等她的姐姐林雪从对岸回来。" Run 1 and Run 2 are in two new Conversations. |

## Plan

Written at 05:21 UTC, before the first request.

- Purpose: show that the real model loads the Skill of the catalog that fits the author request, and that the route accepts the section `Skill instruction` of a selected Skill. Two Runs with the two fixture Skills of S6-01a (`dialogue-style-guide` and `internal-comms`).
- Run 1: an author request for a weekly progress report of the writing work to the editor, with no selection. Expected: the model calls `load_skill` for `internal-comms` and not for `dialogue-style-guide`. Requests: the decision request of RunStep 0 (the `load_skill` call), the request of RunStep 1 with the load output (`finish`), and the closing request (the answer).
- Run 2: an author request for a short dialogue scene, with a `$` selection of `dialogue-style-guide`. Expected: the section `Skill instruction` in RunStep 0, and an answer that uses the corner quotes 「」 of rule 1 of that Skill. Requests: the decision request of RunStep 0 (`finish`), the closing request, and one more request if the model calls a Tool first.
- Planned requests: 6, at most 9 (one more send of a step with a clearer message is in this count). Automatic requests count.
- Expected reported input of each request: about 2,800 tokens for the first request of a Run (Host instructions, six Tools, the catalog, the Chapter), plus about 500 tokens for the load output of `internal-comms` and about 900 tokens for the section of `dialogue-style-guide`, plus the earlier items of the Run for each later request: Run 1 about 2,800, 3,500, and 3,700; Run 2 about 3,700, 4,000, and 4,300. Total about 22,000 tokens. The hard limit of the ticket is 100,000 reported input tokens.
- Request guard: the triggers of the schema `live_guard` of the record of 2026-11-14 with a cap of 9 model requests and 2 Exa exchanges. Above the cap, the trigger refuses the claim transaction, so nothing goes out.
- Correction after the run: each request declared seven Tools (`web_search` and `web_fetch` too), not six. The estimates did not change.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. `prev` is the request whose response the request continued with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 05:23:16 | 1, 0 | `resp_02179472019714958818742e03404164f2bdc6cf7ab386a5d9949` | none | required | one `load_skill` call for `internal-comms` | 2,465 / 227 |
| M2 | 05:23:22 | 1, 1 | `resp_021794720202518cfacb7e54fe5570d0ae3d0783d99a770a4ef4b` | M1 | required | `finish` | 3,058 / 685 |
| M3 | 05:23:39 | 1, 2 | `resp_021794720219326746abb8ea69ccb3d7a75bd7d82f8063f950f7c` | M2 | none | final answer | 3,793 / 245 |
| M4 | 05:24:26 | 2, 0 | `resp_02179472026711972ea91cebe75e8e291705b4c0b59095e3e54d5` | none | required | `finish` | 2,801 / 1,745 |
| M5 | 05:25:02 | 2, 1 | `resp_021794720303038662416cb134b03a75ced108817244360dd72e2` | M4 | none | final answer | 4,612 / 256 |

Total: **5 model requests, 16,729 reported input tokens, and 3,158 output tokens.** The plan was 6 requests and about 22,000 input tokens: Run 2 needed no Tool call before `finish`. No Exa exchange, no Retrieve, no recovery, and no refused call occurred. Each Model Attempt is `settled`, and each Run is `completed`.

## Run 1: the model loads the Skill that fits

Author request, with no selection: "请帮我给编辑写一份本周的写作进度周报，按进展、计划、问题三部分来写：本周写完了《雨夜》这一章的开头；下周计划写完林舟和林雪在渡口重逢的那一场；目前的问题是这一章的节奏偏慢。直接在回答里给我周报全文，不要修改正文。"

The section `Skill catalog` of M1, as the route got it (between `Author request` and `Working text`):

```text
Skill catalog:
{"description":"小说对白的写法规范。当作者要求写、改写或润色人物对白，或者要求检查一段对话是否自然时，使用这个技能。","name":"dialogue-style-guide"}
{"description":"A set of resources to help me write all kinds of internal communications, using the formats that my company likes to use. Claude should use this skill whenever asked to write some sort of internal communications (status reports, leadership updates, 3P updates, company newsletters, FAQs, incident reports, project updates, etc.).","name":"internal-comms"}
```

The Host instructions of M1 to M5 had the paragraph of ADR 0089 section 3 that starts "The section Skill catalog lists the Skills that you can load".

The `load_skill` arguments of the model in M1:

```json
{"skill": "internal-comms", "reason": "作者要求写一份给编辑的写作进度周报，属于状态报告类内部沟通，该技能提供公司偏好的格式与写法规范。"}
```

The model did not call `load_skill` for `dialogue-style-guide`. The load settled with 1,511 bytes. M2 sent one `function_call_output` for the call `call_3e791sxr4aqeoan7dsyreyoc`: the exact bytes of `fixtures/skill-packages/internal-comms/SKILL.md` (byte comparison: equal), with no tag and no escape. M3 sent the empty output of `finish`.

The answer of M3 is a weekly report with the three parts 一、进展, 二、计划, and 三、问题, and no edit of the Chapter.

## Run 2: a selected Skill

The author typed `$` in the composer, chose `dialogue-style-guide` in the picker, and sent a request for eight to ten lines of dialogue. The Operation Requirement of RunStep 0 recorded `[{"provenance": "author_selected", "skill_name": "dialogue-style-guide", "package_digest": "sha256:e03399e0…"}]`.

The user message of M4 had the sections `Author request`, `Skill instruction`, `Skill catalog`, `Working text`, and `Target blocks`, in this order. The section `Skill instruction` was the heading and the exact bytes of `fixtures/skill-packages/dialogue-style-guide/SKILL.md` (1,411 bytes, byte comparison: equal). The route accepted M4 and M5.

Rule 1 of the Skill: all dialogue uses the corner quotes 「」, nested 『』, and never “” or "". Rule 2: the tag is only 说, 问, or 答, after the line. The answer of M5 has nine lines of dialogue, and each uses 「」 and a tag of rule 2. The first four lines:

```text
「船到了。」林舟说。
林雪提着包袱走上跳板，看见她，脚步停了一下。
「你怎么在这儿？」林雪问。
「等你。」林舟说。
```

The answer has no “” and no "". Its last paragraph names the rules that it followed.

## Package digests

| Skill | Package digest | Pinned in |
| --- | --- | --- |
| `dialogue-style-guide` | `sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b` | Run 1 and Run 2 |
| `internal-comms` | `sha256:3ccc24f2e8e7c47369c7a2cf8456271c6338b8a83d1917756c3584b2466c67e9` | Run 1 and Run 2 |

The Project has one Snapshot of each package. The catalog of each Run pinned the two references.

## Key check

The Server log (101 lines), the Worker log (7 lines), the five wire payloads, and a data dump of the `storyos` and `live_guard` schemas (991 lines) have no `Authorization` header, no bearer token, and no `x-api-key`. The matches of the check are the column `qualification_authorization` and the Credential References `settings:STORYOS_AGENT_PLAN_API_KEY` and `settings:STORYOS_EXA_API_KEY`.

## Findings

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| F1 | The `SKILL.md` of `internal-comms` tells the model to read a file of `examples/`. The reasoning summary of M2 says that the model cannot read that file, so it used the 3P format of the author request. | The answer is correct, but the model used only `SKILL.md`. `read_skill_resource` gives the read. | [#1532](https://github.com/FrankQDWang/StoryOS/issues/1532) |
| F2 | After the choice of a Skill in the `$` picker, the composer showed its placeholder, and the text that the browser tool typed next was not visible. A second typing went into the middle of the first text, so the sent author message has the request two times, interleaved. The model followed the request. | Possibly a composer defect after a chip, or an effect of the automated typing. Not examined. | Observation for the coordinator. |
| F3 | In M2 and M4 the model wrote its reasoning and the full text before the `finish` call (685 and 1,745 output tokens), and the closing request wrote the answer again. | Output tokens of about one answer more for each Run. | Observation. |
| F4 | The panel names no Skill that a turn used. | Expected before S6-03f. | [#1533](https://github.com/FrankQDWang/StoryOS/issues/1533) |
