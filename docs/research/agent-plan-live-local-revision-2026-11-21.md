# Agent Plan live run of the SkillPackage local-revision (2026-11-21)

This record is the live evidence of [S10-04: Add the SkillPackage local-revision](https://github.com/FrankQDWang/StoryOS/issues/1764) for `S10-EVD-002` and [ADR 0100](../adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md) section 2. It shows the effect of the instructions of the package `local-revision` on the real Agent Plan route. The run is no trial that compares models or prompts, and it does not assert that the model obeys the text.

Raw evidence is on the tag `evidence/1764-live-local-revision-2026-11-21`. The tag holds the plan, the Chapter, the Project Instruction, the list of the planted habits, the run script, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M8.json`), the Model Attempts, the ToolCalls, the guard log, `measures-A.json` and `measures-B.json` (the Blocks, the Proposal operations, and each answer in full), `checks.json`, `loader.log`, and the Server and Worker logs. The tag has no database dump and no Project Export Archive.

## Plan and final count

The plan comes before the results. The planned numbers are 6 requests in total, 3 for each Run, and about 45,000 reported input tokens. The script of this run stopped only at the database cap of 12 requests, and not at the planned number. Thus the 4th request of each Run, and the reported input above 45,000 tokens, did not stop the run. The stop rule of "Live model requests" that the coordinator set after this run (decision on [#1762](https://github.com/FrankQDWang/StoryOS/issues/1762#issuecomment-7060590530)) stops a run at the first request above the planned number.

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1764#issuecomment-7060564582)). A rehearsal of all steps on `host_fake` on the same head came first: 2 author Runs, 4 fake requests, and no other Run.

| Item | Planned | Actual |
| --- | --- | --- |
| AgentRuns | 2 (limit 4) | 2, each `completed`, each in a new Project and Conversation |
| Model requests | 6 (3 for each Run); the database cap was 12 | 8 (4 for each Run, finding F2) |
| Reported input | about 45,000 tokens | 69,190 tokens |
| Largest request | at most 40,000 | 10,746 |
| Other Runs (Memory work) | 0 | 0 |
| Exa and embedding requests | 0 | 0 |

## Findings

| ID | Finding | Effect |
| --- | --- | --- |
| F1 | **Each Run sent one Proposal in its first `propose_edit` call, with one operation for each changed Block.** Run A changed 5 Blocks, and Run B changed 2 Blocks. Each operation waits for the decision of the author, so the author can accept, change, or refuse each change alone. | The count rules and the rule "one Proposal" show in the first call. |
| F2 | **After the first Proposal, the Host sent a new input with fewer target Blocks, and each Run made a second `propose_edit` call that the Host refused with `after_proposal`.** The user message of the next request said "The input of this author request changed after your last step", and its target Blocks had no Block of the open Proposal. The model read this as a new request: in Run A it proposed Block 8 "in the new five target Blocks", and in Run B it proposed Blocks 2 and 4 as a new set of "2 places". | Each Run used 4 requests and not 3. The total was 8 requests against 6 in the plan, in the cap of 12. The refusal kept the count: no Run changed more Blocks than the instructions permit. |
| F3 | **The final answers do not have the form of the instructions.** Run A gave one line only for the refused change of Block 8 ("然而，雾越来越浓…… —— 段首“然而”连接词＋……三连排比，属机器腔"), and named the 5 changed places in one sentence ("此前那份待决 Proposal 已覆盖另外五处更明显的机器腔（开头的“微微／缓缓”、眼中闪过、石子投湖比喻、心中暖流、命运齿轮）"). Run B gave lines for Blocks 2 and 4, which it did not change, and offered to send them later ("之后我立刻按当前选段提交下面这 2 处"). Neither answer gave one line for each changed place. | A cause is F2: the answer spoke of the second input ("这次的五个目标段里", Run A). |
| F4 | **Block 5, which has the device of the sample passage, stayed unchanged in each Run.** The answer of Run A said why: "样文本身两次使用这一句式……属于你指定的口吻，故不动。" | The rule "what you leave" shows. |
| F5 | **The Blocks with no habit (1, 3, and 7) stayed unchanged in each Run.** | The rule "all other text stays as it is" shows. |
| F6 | **Some candidates add a small action or a sensory detail in the place of the removed habit**: 别开脸 and 没接话 (Block 4), 林舟没接话 and 河水拍着船帮 (Block 6), 握紧那把钥匙 (Block 9), and a new last image (Block 10 of each Run). No candidate adds an event of the plot or a name, and no candidate changes a fact of the Chapter. | Observation for the author. The rule "add no fact of the plot" did not stop small details of the scene. |
| F7 | **The two Runs chose different places first.** Run A chose Blocks 2, 4, 6, 9, and 10, and left Block 8 for its refused second call. Run B, with "只改最明显的 2 处", chose Blocks 9 and 10. | Observation: the order of "the most obvious place" is the choice of the model. |
| F8 | **The run used the mapping `v10` and Registration `fa24`.** The Host instructions had the default rules for prose of #1761, and the instructions of the package name those rules as the habits. Each explanation of a change names a habit of those rules. | The habits of the default rules are sufficient for the model to find the 6 planted habits that it changed. |

## Setup

| Item | Value |
| --- | --- |
| Product code | `1764-local-revision` at `544727597ddd821446403fd633300be5518c7f5e` (tree `e9f016cf97115f5fb9fa04bbff09c030dba58d06`), on `stage10-author-voice` `e27a95fb`. |
| Release package | `python3 scripts/package-release.py` on the clean commit, manifest `sha256:fd1c03ab8818f17eae6d628b431a586be6cf628f71e2957744801d5f11d22083`. `skill-packages/local-revision/SKILL.md` of the release package has the bytes of the tracked file (SHA-256 `13b982e7f579330603794547e910aedcc0aee4682ae0a942e9202e9a400513fb`). |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_SKILLS_DIR` set to `target/release-package/skill-packages`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package. Each process environment had only the named variables, `PATH`, and `HOME`. It had no embedding destination and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh with` database. The only direct write was the guard schema `live_guard`. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa24`, mapping `storyos.volcengine-agent-plan.responses.create.v10`. |
| Script | `local-revision-live.ts`, started by `run.sh` and `rehearse.sh`. It is the run script of #1763 with the steps of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL. |
| Skill | `local-revision`, package digest `sha256:aedcb026ab7ba53db958a3665898dcee83bad3fd1a197f13079d5545afe99a20`. `listSkills` gave it and `conflict-check`, each with the description of its `SKILL.md`. |

## Fixed inputs

Two new Projects 夜渡 A and 夜渡 B, each with assistance on, the Volume 第一卷, and the Chapter 夜渡 of 10 Blocks, written through the public commands. The Chapter was the current Chapter, so it was the Working text of each Run. The text is `chapter-01.md` of the tag.

Project Instruction of each Project: "写作要求：第三人称，贴近林舟的视角，句子短。新写和修改都照下面这段样文的口吻。" and a sample passage that uses the contrast frame 不是……是…… two times on purpose.

| Block | Rule | Planted habit | Text |
| --- | --- | --- | --- |
| 1 | none | none | 天还没亮，林舟就到了渡口。老周的木船拴在桩上，船头挂着一盏防风灯。 |
| 2 | 5 | 微微、缓缓、轻轻 on each action; 仿佛 | 老周微微点头，缓缓起身，轻轻地推开船舱的门，仿佛早就知道他会来。 |
| 3 | none | none | 林舟把两文钱放在船板上，说要去对岸找姐姐。老周没有收钱，只把缆绳解开了。 |
| 4 | 4 | 眼中闪过一丝；嘴角勾起一抹 | 听到“对岸”两个字，老周的眼中闪过一丝慌乱，嘴角勾起一抹苦涩的弧度。 |
| 5 | 1 | 不是……而是…… (the device of the sample: leave it) | 这不是一次普通的渡河，而是林舟第一次违背姐姐的嘱咐。 |
| 6 | 6 | 如同石子投入湖面，泛起层层涟漪 | 老周低声说了一句：“那边的人，回不来了。”这句话如同石子投入湖面，在林舟心里泛起层层涟漪。 |
| 7 | none | none | 船到河心，林舟摸了摸脖子上的怀表。表还停在四点十分。 |
| 8 | 7 | 然而 at the start; three parallel items | 然而，雾越来越浓，水越来越急，岸越来越远。 |
| 9 | 3 | 心中涌起一股暖流；他终于明白 | 林舟心中涌起一股暖流，他终于明白，姐姐留下钥匙，是想让他自己做决定。 |
| 10 | 2 | 他不知道的是；命运的齿轮 | 他不知道的是，命运的齿轮已经开始转动。 |

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. Each Model Attempt is `settled`. `prev`: the request continued the response of the row before it with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 09:28:01 | A, 0 | `resp_021795253281345ab257ece25bd0fea81c82ca5c15ec8fd4b550b` | none | required | `propose_edit` (5 changes) | 5,994 / 4,702 |
| M2 | 09:29:23 | A, 1 | `resp_021795253363876bdd5db804b0f31a919b8f9d8940b3f7fab57cc` | M1 | required | `propose_edit`, refused | 7,999 / 2,051 |
| M3 | 09:29:59 | A, 2 | `resp_02179525340005958818742e03404164f2bdc6cf7ab386a6e7fbd` | M2 | required | `finish` | 10,118 / 576 |
| M4 | 09:30:13 | A, 3 | `resp_021795253413377f9405f279a0f30d2c73a697bf053f75a2ff547` | M3 | none | final answer | 10,746 / 279 |
| M5 | 09:30:24 | B, 0 | `resp_02179525342482216e38debc86f845189b93b52559f1b788a1826` | none | required | `propose_edit` (2 changes) | 5,994 / 3,576 |
| M6 | 09:31:27 | B, 1 | `resp_021795253487632e41c82dd1debd95d24831350e3c3a87766b24a` | M5 | required | `propose_edit`, refused | 7,894 / 1,917 |
| M7 | 09:32:04 | B, 2 | `resp_021795253524559d2ec6088cd57f16be69183642d63bc77f31241` | M6 | required | `finish` | 9,881 / 632 |
| M8 | 09:32:19 | B, 3 | `resp_021795253539448bbc1b38287ece59059a49659ad58bad981c641` | M7 | none | final answer | 10,564 / 395 |

Total: **8 model requests, 69,190 reported input tokens, and 14,128 output tokens** (Run A: 34,857 / 7,608; Run B: 34,333 / 6,520). No Retrieve and no recovery occurred.

Each request declared the same 13 functions: `web_search`, `web_fetch`, `propose_edit`, `submit_research`, `read_tool_result`, `load_skill`, `read_skill_resource`, `read_chapter`, `search_manuscript`, `write_memory`, `read_memory_entry`, `search_memory`, and `finish`. Each request had the section `Skill catalog` with the lines of `conflict-check` and `local-revision`. The instructions of each request had the default rules for prose.

The user message of M2 and of M6 started with "The input of this author request changed after your last step. This input replaces the earlier input of this author request". Its target Blocks were the Blocks of the Chapter with no open Proposal operation: Blocks 1, 3, 5, 7, and 8 in M2, and Blocks 1 to 8 in M6 (finding F2). The function output of the first `propose_edit` in each was "StoryOS opened the Proposal of this change, and the Proposal waits for the decision of the author." The function output of the refused call in M3 and M7 was "The Host refused this call because this Run already opened a Proposal that waits for the decision of the author, so no Proposal opened."

## Run A: the selected Skill, "请修改这一章。"

Author request in Project A, with the selection of `local-revision` in `skill_selections`: "请修改这一章。" The user message of M1 had the section `Skill instruction` with the exact bytes of `skill-packages/local-revision/SKILL.md` (byte comparison: equal).

**Count of changed Blocks: 5** (Blocks 2, 4, 6, 9, and 10), in one Proposal with 5 operations. The summary of the call: "挑出本章五处最明显的机器腔，改为样文式的短句子白描，其余文字不动。"

| Block | Target text | Candidate | Lost | Added |
| --- | --- | --- | --- | --- |
| 2 | 老周微微点头，缓缓起身，轻轻地推开船舱的门，仿佛早就知道他会来。 | 老周点点头，起身，推开船舱的门，像早就知道他会来。 | none (only the habit) | none |
| 4 | 听到“对岸”两个字，老周的眼中闪过一丝慌乱，嘴角勾起一抹苦涩的弧度。 | 听到“对岸”两个字，老周怔了一下，别开脸，没接话。 | the words 慌乱 and 苦涩 (the feeling now shows through 怔了一下) | the actions 别开脸 and 没接话 |
| 6 | 老周低声说了一句：“那边的人，回不来了。”这句话如同石子投入湖面，在林舟心里泛起层层涟漪。 | 老周低声说了一句：“那边的人，回不来了。”林舟没接话。河水拍着船帮。 | the effect of the words on 林舟 (only the habit) | the action 林舟没接话 and the detail 河水拍着船帮 |
| 9 | 林舟心中涌起一股暖流，他终于明白，姐姐留下钥匙，是想让他自己做决定。 | 林舟握紧那把钥匙。姐姐留下它，是让他自己拿主意。 | the warm feeling and 终于明白 (only the habit) | the action 握紧那把钥匙 |
| 10 | 他不知道的是，命运的齿轮已经开始转动。 | 船往雾里走，看不见对岸。 | the foreshadowing (only the habit) | a new last image: the boat goes into the fog, and the other bank is not visible |

Each explanation names the habit, for example "原句叠用“微微、缓缓、轻轻地、仿佛”四个修饰词，是典型机器腔" (Block 2) and "是规则中点名的惯用比喻" (Block 6). No candidate changes a name, an event, or the sequence of the information.

**Unchanged Blocks:** 1, 3, 5, 7, and 8. **Block 5, with the device of the sample passage, stayed unchanged.** Block 8 has a planted habit. The model proposed "雾更浓了，水急起来，岸看不见了。" for it in M2, with the summary "在新的五个目标段中，只改“然而，雾越来越浓……”这一处", and the Host refused the call (finding F2).

**Lines of the final answer:**

1. 这次的五个目标段里，只有一处我建议改，但没能成功提交：当前已有一份 Proposal 在等你处理，系统不让同时再开一份……
2. 然而，雾越来越浓…… —— 段首“然而”连接词＋“雾越来越／水越来越／岸越来越”三连排比，属机器腔。建议改为：“雾更浓了，水急起来，岸看不见了。”……
3. “天还没亮……”“林舟把两文钱……”“船到河心……”三段都是短句子的白描，贴近林舟视角，没有需要改的毛病。
4. “这不是一次普通的渡河，而是……”虽是“不是……而是……”句式，但样文本身两次使用这一句式……属于你指定的口吻，故不动。
5. 另：此前那份待决 Proposal 已覆盖另外五处更明显的机器腔（开头的“微微／缓缓”、眼中闪过、石子投湖比喻、心中暖流、命运齿轮），请先逐条接受或拒绝。

The full answer is in `measures-A.json`. The answer speaks of the second input of M2 ("这次的五个目标段里，只有一处我建议改"). It gives no line for each of the 5 changed places in the form of the instructions (finding F3).

## Run B: the selected Skill, "只改最明显的 2 处。"

Author request in Project B, with the selection of `local-revision`: "只改最明显的 2 处。" Project B had no Proposal before the Run.

**Count of changed Blocks: 2** (Blocks 9 and 10), in one Proposal with 2 operations:

| Block | Target text | Candidate |
| --- | --- | --- |
| 9 | 林舟心中涌起一股暖流，他终于明白，姐姐留下钥匙，是想让他自己做决定。 | 姐姐留下钥匙，是让他自己拿主意。 |
| 10 | 他不知道的是，命运的齿轮已经开始转动。 | 雾漫上来，船还在往对岸走。 |

The candidate of Block 9 lost the name 林舟 at the start of the Block. The candidate of Block 10 added a new last image. In M6 the model proposed Blocks 2 and 4 with the summary "改最明显的 2 处机器腔", and the Host refused the call (finding F2). The final answer said that it cannot send a new suggestion while a Proposal waits ("现在没法提交新建议：这一章已有一条待处理的 Proposal"), gave the two refused changes of Blocks 2 and 4 to send after the decision of the author ("请先接受或拒绝那条待处理建议；之后我立刻按当前选段提交下面这 2 处"), and said that 3 more places stay ("选段里另外还留着 3 处较明显的（“不是普通渡河，而是……”“如同石子投入湖面……”“然而，雾越来越浓……”），这次按你说的不动。"), which are Blocks 5, 6, and 8. Block 5 stayed unchanged.

## Key check

The Server log (121 lines), the Worker log (10 lines), the eight wire payloads, and the other files of the tag have no `Authorization` header, no bearer token, no `x-api-key`, no e-mail address, and no home path.
