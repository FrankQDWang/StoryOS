# Unresolved passage request live run (2026-11-14)

This record is the live evidence of [An Edit Message With a Passage Request That Does Not Resolve Gets No Working Text](https://github.com/FrankQDWang/StoryOS/issues/1496) for the specification [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451). It records one Run on the real Agent Plan route with the message of Run 3 of the [tool loop live run](agent-tool-loop-live-run-2026-11-14.md) (finding F1).

## Setup

| Item | Value |
| --- | --- |
| Product code | `claude/issue-1496-unresolved-passage-working-text` at `7644967b8433bf1411c79dd21d1decda6cfb4b66` (tree `b2da28b9`), from `agent-tool-loop` at `690ce0cf`. |
| Release package | `python3 scripts/package-release.py` on the clean commit above, manifest `sha256:8aee85026c0e9d21d06e1c00ccbb99c1042b7ec1193ad5178cd06dc58c1771bb`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8745`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package. |
| Settings | ADR 0050: the Worker resolved the keys from the env file of the main checkout. This run never read, copied, or printed that file or a key. The Server log (114 lines), the Worker log (4 lines), and a data dump of the `storyos` schema have no `Authorization` header, no bearer token, and no `x-api-key`. The one match of "authorization" in the dump is the column `qualification_authorization`. |
| Database | New `scripts/dev-postgres.sh up --interactive` database of this worktree with the controlled fixture. The only direct writes were the guard schema `live_guard`. The Chapter text went through the editor. The database was removed after the run. |
| Request guard | Triggers of the guard schema: the insert of a Model Attempt counts one model request, and the first `destination_attempt_id` of a ToolCall counts one Exa exchange. Above the cap the trigger refuses the claim transaction, so nothing goes out. |
| Model | `doubao-seed-2-1-pro-260915`, mapping `storyos.volcengine-agent-plan.responses.create.v5`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`. `localStorage`, `sessionStorage`, and IndexedDB of the origin were clear before the first step. New Project `01a1d7c1-9f13-70e2-9e8e-b26348c6fd37` ("未解析段落实测"), Volume 第一卷, Chapter 灯塔 `01a1d7c2-1e2b-7e92-b695-a71235825295` with three paragraphs, typed in the editor. |

Chapter text, one paragraph for each line:

```text
雾从海面升起来的时候，灯塔的灯已经照不远了。
老守塔人说过，雾航的规矩是：只要雾不散，钟就不能停。
林三爬上钟楼，抓住绳子，开始敲钟。
```

## Plan

Written at 2026-11-14T01:51:37Z, before the first request.

- Purpose: show that the message of Run 3 of the record of 2026-11-14 gets the working text and the target blocks of the Current Chapter on the real route, and that a `propose_edit` call opens a Proposal.
- One Run, edit: `propose_edit`, `finish`, then the closing request. Plan: 3 model requests, 0 Exa exchanges.
- Automatic requests (a Retrieve, a compaction request, a correction after a refusal) count.
- Hard cap: 8 model requests, 0 Exa exchanges. The guard refuses each claim above the cap.

Final count: **3 model requests and 0 Exa exchanges**, equal to the plan. No Retrieve, no compaction request, and no recovery occurred.

## Live request log

Each row is one claim that the guard counted. The time is the claim time (UTC). The request ID is the complete response reference of the Provider. Usage is input / output tokens as the Provider reported them. `prev` is the end of the `previous_response_id` of the request.

| # | Time | RunStep | Request ID | Mapping, `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 01:53:38 | 0 | `resp_02179462121870493e048b1e87f4ca8485d3a0240faade740d74a` | none, none | required | 1 `propose_edit` call on the third block, settled, Proposal opened | 1722 / 504 |
| M2 | 01:53:58 | 1 | `resp_0217946212382806d6c4a2c22a8ac0107afc732836cd4227a9483` | full, none | required | `finish` counted | 2042 / 36 |
| M3 | 01:54:00 | 2 | `resp_0217946212407421cb6d3ad1bdd7a7436196dd041d7fa4251eb02` | incremental, M2 | none | final answer | 2129 / 237 |

Usage total: 5,893 input tokens and 777 output tokens in 3 model requests.

## Run

| Item | Value |
| --- | --- |
| Run | `01a1d7c2-9e14-7c02-ba46-71bd4f124847`, Conversation `01a1d7c2-9e14-7c02-ba46-71a576a4986c`, `completed` |
| Author message | "请修改第三段：把林三敲钟的动作写得更具体，写出他的手和呼吸，并按雾航规则的意思让他一直敲下去。只改这一段，不要联网。" |
| Context Assembly | Each of the 3 RunSteps selected `author_instruction` (58 tokens) and `working_target` (67 tokens), and later the Tool results. The record keeps the disposition `clarification` of the passage parser. In the tool loop live run, Run 3 had `working_target` 0 tokens. |
| Request M1 | The sent content has "Working text:" with the three paragraphs and "Target blocks:" with the three block identities `01a1d7c2-1e30-…`, `01a1d7c2-4912-…`, `01a1d7c2-4923-743a-9122-852b6a8a1510`. |
| Tool calls | RunStep 0: `propose_edit`, `settled`, no refusal, result "StoryOS opened the Proposal of this change, and the Proposal waits for the decision of the author." RunStep 1: `finish`, `settled`. No `web_search` or `web_fetch` call. |
| Proposal | `01a1d7c2-eb3c-7b82-bde2-38bab77fdf70`, `block_edit`, one Operation on block `01a1d7c2-4923-743a-9122-852b6a8a1510` (the third paragraph), `pending`, `unresolved`. No Operation on another block. |
| Panel | The Host sentence "这次修改涉及 1 个章节、1 处文字，具体建议在下方。", the change location 灯塔 · 第1处修改 with its explanation, and the final answer text. The editor shows the candidate under the third paragraph with 接受 and 拒绝. |

Candidate text of the third paragraph (model output):

```text
林三爬上钟楼，攥住那根粗绳。麻绳磨着掌心，他十指用力，指节绷得发白；吸一口气，沉下肩膀，便一下一下拉动绳锤。钟声破雾荡开，他的呼吸跟着绳起绳落，一口一口粗重起来，白气在唇边散开。手心磨出了汗，他换只手再攥；胳膊酸得发抖，他也只是喘口气，接着敲。雾不散，钟就不停。
```

- The model changed only the third paragraph, as the author asked. It did not call a research Tool, as the author asked ("不要联网").
- The Run ended as the finish rule expects: each request before the counted `finish` had `tool_choice: "required"`, the closing request had `tool_choice: "none"`, and the closing response held no function call.

## Observations

| ID | Observation | Owner |
| --- | --- | --- |
| O1 | F1 of the tool loop live run does not occur: the message of Run 3 gets the working text and the target blocks of the Current Chapter, and the first `propose_edit` call opens a Proposal. Run 3 of that record used 5 requests and gave no Proposal. This Run used 3 requests and gave a Proposal. | #1496 |
| O2 | After the `propose_edit` call of M1 opened the Proposal, M2 used the Full mapping and no prior response. This is finding F2 of the tool loop live run. It has no effect on this Run, because the Conversation had no earlier turn. | #1497 |
